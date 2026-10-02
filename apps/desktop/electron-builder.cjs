const path = require('node:path');
const fs = require('node:fs');
const runtimeRoot = path.resolve(process.env.FILEFORM_RUNTIME_ROOT || path.join(__dirname,'../../Artifacts/DesktopRuntime'));
const runtime = JSON.parse(fs.readFileSync(path.join(runtimeRoot,'runtime.json'),'utf8'));
if(runtime.schemaVersion !== 1 || runtime.platform !== process.platform || runtime.architecture !== (process.arch==='arm64'?'aarch64':process.arch==='x64'?'x86_64':process.arch)) throw new Error('Stage the complete native runtime for this platform before packaging.');
module.exports = {
  appId:'app.fileform.DesktopPreview',productName:'Fileform Preview',
  directories:{output:'artifacts'},
  files:['dist/**/*','dist-main/**/*','package.json'],
  extraResources:[{from:runtimeRoot,to:'native'},{from:path.resolve(__dirname,'../../Artifacts/desktop-notices'),to:'notices'}],
  mac:{target:['dir'],identity:null,minimumSystemVersion:runtime.minimumMacOS??'14.0',category:'public.app-category.utilities',icon:'../macos/Resources/Assets.xcassets/AppIcon.appiconset/icon-512@2x.png'},
  win:{target:['nsis'],icon:'../macos/Resources/Assets.xcassets/AppIcon.appiconset/icon-512@2x.png'},
  npmRebuild:false,asar:true,
  electronFuses:{runAsNode:false,enableNodeOptionsEnvironmentVariable:false,enableNodeCliInspectArguments:false,enableEmbeddedAsarIntegrityValidation:true,onlyLoadAppFromAsar:true,grantFileProtocolExtraPrivileges:false,resetAdHocDarwinSignature:true}
};
