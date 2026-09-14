import assert from 'node:assert/strict';
import test from 'node:test';
import {validateImageExport,outputDimensions,parseByteLimit} from '../src/image-export.ts';
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

test('decimal byte limits are exact and fitting validates floors',()=>{
 assert.equal(parseByteLimit('6.63','KB'),6630);
 assert.equal(parseByteLimit('.000001','MB'),1);
 assert.equal(parseByteLimit('536.870912','MB'),536870912);
 assert.equal(parseByteLimit('536.870913','MB'),undefined);
 for(const value of ['','0','-1','1e6','1.5','NaN'])assert.equal(parseByteLimit(value,'B'),undefined);
 assert.equal(parseByteLimit('0.0001','KB'),undefined);
 assert.equal(validateImageExport({format:'jpeg',background:'white',quality:85,minimumQuality:35,maxBytes:6630},12,16,true).maxBytes,6630);
 for(const value of [{format:'png',maxBytes:0},{format:'png',maxBytes:1.5},{format:'jpeg',background:'white',quality:40,minimumQuality:50,maxBytes:1000},{format:'jpeg',background:'white',minimumQuality:35},{format:'tiff',minimumQuality:35,maxBytes:1000}])assert.throws(()=>validateImageExport(value,12,16,true));
});
