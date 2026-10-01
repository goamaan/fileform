const path = require('node:path');
const base = require('./electron-builder.cjs');
const identity = process.env.FILEFORM_SIGNING_IDENTITY;
// Full-runtime signing must regenerate nested pack manifests after signing.
// Do not silently use the earlier worker-only signature recipe on this bundle.
throw new Error('Full native-runtime signing and manifest refresh must be completed before a signed release build.');
if (process.platform !== 'darwin' || !identity?.trim()) {
  throw new Error('Signed macOS builds require FILEFORM_SIGNING_IDENTITY and a Mac.');
}
module.exports = {
  ...base,
  forceCodeSigning: true,
  mac: {
    ...base.mac,
    identity: identity.replace(/^Developer ID Application:\s*/, ''),
    hardenedRuntime: true,
    entitlements: path.join(__dirname, 'build/entitlements.mac.plist'),
    entitlementsInherit: path.join(__dirname, 'build/entitlements.mac.plist'),
    preAutoEntitlements: false,
    binaries: ['Contents/Resources/native/fileform-worker'],
    // Notarization is a separate, verifiable step using a Keychain profile.
    notarize: false,
  },
};
