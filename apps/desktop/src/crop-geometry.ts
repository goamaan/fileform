import type {PixelCrop} from './contracts';
export type Corner='nw'|'ne'|'sw'|'se';
const clamp=(value:number,min:number,max:number)=>Math.min(max,Math.max(min,Math.round(value)));
export function resizeCrop(start:PixelCrop,corner:Corner,dx:number,dy:number,width:number,height:number):PixelCrop {
  const left=corner.endsWith('w')?clamp(start.x+dx,0,start.x+start.width-1):start.x;
  const right=corner.endsWith('e')?clamp(start.x+start.width+dx,start.x+1,width):start.x+start.width;
  const top=corner.startsWith('n')?clamp(start.y+dy,0,start.y+start.height-1):start.y;
  const bottom=corner.startsWith('s')?clamp(start.y+start.height+dy,start.y+1,height):start.y+start.height;
  return {x:left,y:top,width:right-left,height:bottom-top};
}
