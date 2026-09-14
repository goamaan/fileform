import assert from 'node:assert/strict';
import test from 'node:test';
import {resizeCrop} from '../src/crop-geometry.ts';
const start={x:10,y:20,width:30,height:40};
test('corner movement clamps to image bounds without empty selections',()=>{
 assert.deepEqual(resizeCrop(start,'nw',-999,-999,100,80),{x:0,y:0,width:40,height:60});
 assert.deepEqual(resizeCrop(start,'se',999,999,100,80),{x:10,y:20,width:90,height:60});
 assert.deepEqual(resizeCrop(start,'nw',999,999,100,80),{x:39,y:59,width:1,height:1});
 for(const corner of ['nw','ne','sw','se'])for(const dx of [-200,-0.5,0,0.5,200])for(const dy of [-200,0,200]){
  const crop=resizeCrop(start,corner,dx,dy,100,80);
  assert.ok(Object.values(crop).every(Number.isInteger));
  assert.ok(crop.x>=0&&crop.y>=0&&crop.width>=1&&crop.height>=1&&crop.x+crop.width<=100&&crop.y+crop.height<=80);
 }
});
