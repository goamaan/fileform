const path = require('node:path');
const base = require('./electron-builder.cjs');
const identity = process.env.FILEFORM_SIGNING_IDENTITY;
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
