import {spawnSync} from 'node:child_process';
import {mkdtempSync, realpathSync, statSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

// Credentials remain in Keychain. Never accept or print an account password.
const [bundle, profile] = process.argv.slice(2);
if (process.platform !== 'darwin' || !bundle || !profile) {
  throw new Error('Usage: node scripts/notarize-mac.mjs /absolute/App.app KEYCHAIN_PROFILE');
}
const app = realpathSync(bundle);
if (!app.endsWith('.app') || !statSync(app).isDirectory()) throw new Error('Expected an app bundle.');
function run(command, args, capture = false) {
  const result = spawnSync(command, args, {encoding: 'utf8', stdio: capture ? 'pipe' : 'inherit'});
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} failed (${result.status}).`);
  return result.stdout;
}
run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', app]);
// codesign writes signature metadata to stderr; inspect it separately.
const metadata = spawnSync('codesign', ['--display', '--verbose=4', app], {encoding:'utf8'});
if (metadata.status !== 0 || !metadata.stderr.includes('Authority=Developer ID Application:') || !metadata.stderr.includes('runtime')) {
  throw new Error('A Developer ID signature with hardened runtime is required.');
}
const worker = join(app, 'Contents/Resources/native/fileform-worker');
run('codesign', ['--verify', '--strict', '--verbose=2', worker]);
const workerMetadata = spawnSync('codesign', ['--display', '--verbose=4', worker], {encoding:'utf8'});
const team = metadata.stderr.match(/^TeamIdentifier=(.+)$/m)?.[1];
const workerTeam = workerMetadata.stderr.match(/^TeamIdentifier=(.+)$/m)?.[1];
if (workerMetadata.status !== 0 || !team || team === 'not set' || workerTeam !== team) {
  throw new Error('The native worker must be signed by the app publisher.');
}
const folder = mkdtempSync(join(tmpdir(), 'fileform-notarization-'));
const archive = join(folder, 'submission.zip');
run('ditto', ['-c', '-k', '--keepParent', app, archive]);
const submission = JSON.parse(run('xcrun', ['notarytool', 'submit', archive, '--keychain-profile', profile, '--wait', '--output-format', 'json'], true));
console.log(JSON.stringify({id:submission.id, status:submission.status}));
if (submission.status !== 'Accepted') throw new Error('Notarization was not accepted; inspect the submission log.');
run('xcrun', ['stapler', 'staple', app]);
run('xcrun', ['stapler', 'validate', app]);
run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', app]);
run('spctl', ['--assess', '--type', 'execute', '--verbose=2', app]);
console.log('App notarized and stapled. Create distribution archives from this stapled bundle.');
