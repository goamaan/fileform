// SPDX-License-Identifier: Apache-2.0
import {promises as fs} from 'node:fs';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
import type {AssetRecord,Worker} from './assets.cjs';
import type {NativeRuntime} from './runtime.cjs';
import type {PageThumbnail} from '../src/contracts.js';

// Only generated bounded PNGs cross this interface. Source/temporary paths stay
// private, and a preview of changed content is never attached to an older asset.
export async function previewPages(
  pages:{record:AssetRecord;pageIndex:number}[],worker:Worker,runtime:NativeRuntime,temp:string,
):Promise<PageThumbnail[]> {
  if(!pages.length||pages.length>12)throw new Error('Preview up to 12 pages at a time.');
  for(const {record,pageIndex} of pages){
    if(record.asset.family!=='pdf'||!Number.isSafeInteger(pageIndex)||pageIndex<0||pageIndex>=(record.asset.pages??0))throw new Error('Choose an existing PDF page.');
  }
  const folder=await fs.mkdtemp(join(temp,'fileform-pages-'));
  try {
    const thumbnails:PageThumbnail[]=[];
    for(const {record,pageIndex} of pages){
      const output=join(folder,thumbnails.length+'.png');
      const receipt=await worker({operation:'render_pdf_page',input:record.path,output,directory:runtime.pack('renderer'),page_index:pageIndex,max_dimension:240});
      if(receipt.source_sha256!==record.sha256)throw new Error('This file changed. Add it again.');
      if(receipt.output!==output||receipt.page_index!==pageIndex||![receipt.width,receipt.height].every(v=>Number.isSafeInteger(v)&&v>=1&&v<=240)||!Number.isSafeInteger(receipt.bytes)||receipt.bytes<8||receipt.bytes>512_000)throw new Error('Invalid page preview receipt.');
      const png=await fs.readFile(output);
      if(png.length!==receipt.bytes||!png.subarray(0,8).equals(Buffer.from([137,80,78,71,13,10,26,10]))||createHash('sha256').update(png).digest('hex')!==receipt.sha256)throw new Error('The page preview could not be verified.');
      thumbnails.push({sourceID:record.asset.id,pageIndex,dataURL:'data:image/png;base64,'+png.toString('base64'),width:receipt.width,height:receipt.height});
    }
    return thumbnails;
  } finally {await fs.rm(folder,{recursive:true,force:true});}
}
