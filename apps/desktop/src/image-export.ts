import type {ImageExportOptions,PixelCrop} from './contracts.js';
const integer=(value:unknown):value is number=>Number.isSafeInteger(value)&&Number(value)>=0;
export function validateImageExport(value:unknown,width:number,height:number,hasAlpha:boolean):ImageExportOptions {
  if(!value||typeof value!=='object'||Object.keys(value).some(key=>!['format','background','quality','crop','maxDimension'].includes(key)))throw new Error('Invalid image export options.');
  const {format,background,quality,crop:region,maxDimension}=value as Record<string,unknown>;
  if(format!=='png'&&format!=='jpeg')throw new Error('Choose PNG or JPEG.');
  if(quality!==undefined&&(!integer(quality)||quality<1||quality>100))throw new Error('JPEG quality must be between 1 and 100.');
  if(background!==undefined&&background!=='white'&&background!=='black')throw new Error('Choose a white or black background.');
  if(format==='png'&&(quality!==undefined||background!==undefined))throw new Error('Quality and background apply to JPEG output.');
  if(format==='jpeg'&&hasAlpha&&background===undefined)throw new Error('Choose a background for JPEG.');
  if(maxDimension!==undefined&&(!integer(maxDimension)||maxDimension<1||maxDimension>4294967295))throw new Error('Maximum dimension must be a positive whole number.');
  let crop:PixelCrop|undefined;
  if(region!==undefined){
    if(!region||typeof region!=='object'||Object.keys(region).sort().join(',')!=='height,width,x,y')throw new Error('Invalid crop.');
    const item=region as PixelCrop;
    if(![item.x,item.y,item.width,item.height].every(integer)||item.width<1||item.height<1||item.x+item.width>width||item.y+item.height>height)throw new Error('Crop must stay inside the image.');
    crop={x:item.x,y:item.y,width:item.width,height:item.height};
  }
  return {format,background,quality,crop,maxDimension} as ImageExportOptions;
}
export function outputDimensions(width:number,height:number,maximum?:number):{width:number;height:number}{
  const longest=Math.max(width,height);
  if(maximum===undefined||maximum>=longest)return {width,height};
  return {width:Math.max(1,Math.floor((width*maximum+Math.floor(longest/2))/longest)),height:Math.max(1,Math.floor((height*maximum+Math.floor(longest/2))/longest))};
}
