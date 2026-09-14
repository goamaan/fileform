const path = require('node:path');
const suffix = process.platform === 'win32' ? '.exe' : '';
module.exports = {
  appId:'app.fileform.DesktopPreview',productName:'Fileform Preview',
  directories:{output:'artifacts'},
  files:['dist/**/*','dist-main/**/*','package.json'],
  extraResources:[{from:path.resolve(__dirname,'../../target/release/fileform-worker'+suffix),to:'native/fileform-worker'+suffix}],
  mac:{target:['dir'],identity:null,category:'public.app-category.utilities',icon:'../macos/Resources/Assets.xcassets/AppIcon.appiconset/icon-512@2x.png'},
  win:{target:['nsis'],icon:'../macos/Resources/Assets.xcassets/AppIcon.appiconset/icon-512@2x.png'},
  npmRebuild:false,asar:true,
  electronFuses:{runAsNode:false,enableNodeOptionsEnvironmentVariable:false,enableNodeCliInspectArguments:false,enableEmbeddedAsarIntegrityValidation:true,onlyLoadAppFromAsar:true,grantFileProtocolExtraPrivileges:false,resetAdHocDarwinSignature:true}
};
