import assert from 'node:assert/strict';
import test from 'node:test';
import {validateImageExport,outputDimensions} from '../src/image-export.ts';
test('predicted dimensions match integer native rounding and do not enlarge',()=>{
 for(const [width,height] of [[12,16],[10,12],[80000000,1],[1,79999999],[12345,54321]])for(const maximum of [1,8,1280,Math.max(width,height)-1,4294967295]){
  const longest=Math.max(width,height);
  const expected=maximum>=longest?{width,height}:{width:Number((BigInt(width)*BigInt(maximum)+BigInt(Math.floor(longest/2)))/BigInt(longest))||1,height:Number((BigInt(height)*BigInt(maximum)+BigInt(Math.floor(longest/2)))/BigInt(longest))||1};
  assert.deepEqual(outputDimensions(width,height,maximum),expected);
 }
 assert.deepEqual(outputDimensions(10,12,8),{width:7,height:8});
});
test('export validation rejects malformed settings and copies allowed crop values',()=>{
 for(const value of [{format:'png',maxDimension:0},{format:'png',maxDimension:1.5},{format:'png',maxDimension:Infinity},{format:'png',maxDimension:4294967296},{format:'jpeg'},{format:'png',quality:85},{format:'png',background:'white'},{format:'tiff',quality:80},{format:'tiff',background:'white'},{format:'png',path:'/tmp/anything'},{format:'png',crop:{x:0,y:0,width:13,height:1}}])assert.throws(()=>validateImageExport(value,12,16,true));
 assert.equal(validateImageExport({format:'tiff'},12,16,true).format,'tiff');
 const input={format:'jpeg',background:'black',quality:40,maxDimension:8,crop:{x:0,y:2,width:10,height:12}};
 const checked=validateImageExport(input,12,16,true);input.crop.width=1;
 assert.equal(checked.crop.width,10);
 assert.equal(checked.maxDimension,8);
});
