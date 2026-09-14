import type {ImageExportOptions,PixelCrop} from './contracts.js';
const integer=(value:unknown):value is number=>Number.isSafeInteger(value)&&Number(value)>=0;
export function validateImageExport(value:unknown,width:number,height:number,hasAlpha:boolean):ImageExportOptions {
  if(!value||typeof value!=='object'||Object.keys(value).some(key=>!['format','background','quality','crop','maxDimension','maxBytes','minimumQuality'].includes(key)))throw new Error('Invalid image export options.');
  const {format,background,quality,crop:region,maxDimension,maxBytes,minimumQuality}=value as Record<string,unknown>;
  if(format!=='png'&&format!=='jpeg'&&format!=='tiff')throw new Error('Choose PNG, JPEG or TIFF.');
  if(quality!==undefined&&(!integer(quality)||quality<1||quality>100))throw new Error('JPEG quality must be between 1 and 100.');
  if(background!==undefined&&background!=='white'&&background!=='black')throw new Error('Choose a white or black background.');
  if(format!=='jpeg'&&(quality!==undefined||background!==undefined))throw new Error('Quality and background apply to JPEG output.');
  if(format==='jpeg'&&hasAlpha&&background===undefined)throw new Error('Choose a background for JPEG.');
  if(maxDimension!==undefined&&(!integer(maxDimension)||maxDimension<1||maxDimension>4294967295))throw new Error('Maximum dimension must be a positive whole number.');
  if(maxBytes!==undefined&&(!integer(maxBytes)||maxBytes<1||maxBytes>536870912))throw new Error('File-size limit must be between 1 byte and 512 MiB.');
  if(minimumQuality!==undefined&&(format!=='jpeg'||maxBytes===undefined||!integer(minimumQuality)||minimumQuality<1||minimumQuality>(typeof quality==='number'?quality:85)))throw new Error('Minimum quality requires a JPEG byte limit and must not exceed the chosen quality.');
  let crop:PixelCrop|undefined;
  if(region!==undefined){
    if(!region||typeof region!=='object'||Object.keys(region).sort().join(',')!=='height,width,x,y')throw new Error('Invalid crop.');
    const item=region as PixelCrop;
    if(![item.x,item.y,item.width,item.height].every(integer)||item.width<1||item.height<1||item.x+item.width>width||item.y+item.height>height)throw new Error('Crop must stay inside the image.');
    crop={x:item.x,y:item.y,width:item.width,height:item.height};
  }
  return {format,background,quality,crop,maxDimension,maxBytes,minimumQuality} as ImageExportOptions;
}
export function outputDimensions(width:number,height:number,maximum?:number):{width:number;height:number}{
  const longest=Math.max(width,height);
  if(maximum===undefined||maximum>=longest)return {width,height};
  return {width:Math.max(1,Math.floor((width*maximum+Math.floor(longest/2))/longest)),height:Math.max(1,Math.floor((height*maximum+Math.floor(longest/2))/longest))};
}

export type ByteUnit='B'|'KB'|'MB';
export function parseByteLimit(text:string,unit:ByteUnit):number|undefined {
  if(text.length>32)return undefined;
  const value=text.trim();const match=/^(\d*)(?:\.(\d*))?$/.exec(value);
  if(!match||!/[0-9]/.test(value))return undefined;
  const fraction=match[2]??'';const scale=10n**BigInt(fraction.length);
  const multiplier={B:1n,KB:1000n,MB:1000000n}[unit];if(multiplier===undefined)return undefined;
  const amount=(BigInt(match[1]||'0')*scale+BigInt(fraction||'0'))*multiplier;
  if(amount%scale!==0n)return undefined;
  const bytes=amount/scale;return bytes>=1n&&bytes<=536870912n?Number(bytes):undefined;
}
