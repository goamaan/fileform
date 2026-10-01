#!/usr/bin/env python3
"""Assemble owned HEIF metadata cases around existing synthetic HEVC samples.

No user files, external codec encoder or production container parser is involved.
"""
import argparse
from pathlib import Path
import struct
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('output',type=Path)
args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=False)
root=Path(__file__).resolve().parents[1]
def u16(value):return struct.pack('>H',value)
def u32(value):return struct.pack('>I',value)
def box(kind,data):return u32(len(data)+8)+kind+data
def full(kind,data,version=0,flags=0):return box(kind,bytes([version])+flags.to_bytes(3,'big')+data)
def boxes(data,start=0,end=None):
    end=len(data) if end is None else end
    while start<end:
        assert end-start>=8
        size=int.from_bytes(data[start:start+4],'big');kind=data[start+4:start+8];header=8
        if size==1:size=int.from_bytes(data[start+8:start+16],'big');header=16
        if size==0:size=end-start
        assert size>=header and start+size<=end
        yield kind,data[start:start+size],data[start+header:start+size],start
        start+=size

def source(name):
    data=(root/'native/heic-decode/fixtures'/name).read_bytes();top={kind:(raw,payload) for kind,raw,payload,_ in boxes(data)}
    meta={kind:(raw,payload) for kind,raw,payload,_ in boxes(top[b'meta'][1],4)}
    primary=int.from_bytes(meta[b'pitm'][1][4:6],'big')
    iprp={kind:payload for kind,_,payload,_ in boxes(meta[b'iprp'][1])}
    properties=[(kind,raw,payload) for kind,raw,payload,_ in boxes(iprp[b'ipco'])]
    assoc=iprp[b'ipma'];version=assoc[0];wide=int.from_bytes(assoc[1:4],'big')&1;pos=8;mapping={}
    entries=int.from_bytes(assoc[4:8],'big')
    for _ in range(entries):
        width=2 if version==0 else 4;item=int.from_bytes(assoc[pos:pos+width],'big');pos+=width
        count=assoc[pos];pos+=1;values=[]
        for _ in range(count):
            width=2 if wide else 1;value=int.from_bytes(assoc[pos:pos+width],'big');pos+=width
            values.append(value& (0x7fff if wide else 0x7f))
        mapping[item]=values
    iloc=meta[b'iloc'][1];version=iloc[0];osize,lsize=iloc[4]>>4,iloc[4]&15;bsize,isize=iloc[5]>>4,(iloc[5]&15 if version in (1,2) else 0);pos=6
    def number(width):
        nonlocal pos
        value=int.from_bytes(iloc[pos:pos+width],'big');pos+=width;return value
    count=number(2 if version<2 else 4);payloads={}
    for _ in range(count):
        item=number(2 if version<2 else 4);method=number(2)&15 if version in (1,2) else 0
        assert method==0 and number(2)==0
        base=number(bsize);extents=number(2);value=bytearray()
        for _ in range(extents):
            if isize:number(isize)
            offset,length=number(osize),number(lsize);assert base+offset+length<=len(data)
            value.extend(data[base+offset:base+offset+length])
        payloads[item]=bytes(value)
    return top[b'ftyp'][0],meta[b'hdlr'][0],properties,mapping,payloads,primary

def write(name,items,properties,associations,primary,references):
    ftyp,handler,*_=source('srgb.heic')
    info=full(b'iinf',u16(len(items))+b''.join(full(b'infe',u16(item)+u16(0)+kind+b'\0',2,int(hidden)) for item,kind,hidden,payload in items))
    prop=box(b'iprp',box(b'ipco',b''.join(properties))+full(b'ipma',u32(len(associations))+b''.join(u16(item)+bytes([len(values)])+bytes([index|0x80 for index in values]) for item,values in associations.items())))
    refs=full(b'iref',b''.join(box(kind,u16(from_item)+u16(len(to_items))+b''.join(u16(v) for v in to_items)) for kind,from_item,to_items in references))
    def meta(offset):
        cursor=offset;locations=[]
        for item,kind,hidden,payload in items:
            locations.append(u16(item)+u16(0)+u16(1)+u32(cursor)+u32(len(payload)));cursor+=len(payload)
        loc=full(b'iloc',b'\x44\x00'+u16(len(items))+b''.join(locations))
        return full(b'meta',handler+full(b'pitm',u16(primary))+info+loc+prop+refs)
    preliminary=meta(0);metadata=meta(len(ftyp)+len(preliminary)+8)
    assert len(preliminary)==len(metadata)
    (args.output/(name+'.heic')).write_bytes(ftyp+metadata+box(b'mdat',b''.join(item[3] for item in items)))
_,_,properties,mapping,payloads,primary=source('srgb.heic')
base=[raw for kind,raw,payload in properties if kind in (b'hvcC',b'ispe',b'pixi')]
assert len(base)==3
image=(1,b'hvc1',False,payloads[primary])
_,_,p3props,p3map,p3payload,p3primary=source('p3.heic')
p3base=[raw for kind,raw,payload in p3props if kind in (b'hvcC',b'ispe',b'pixi')]
p3image=(1,b'hvc1',False,p3payload[p3primary])
for name,cp,tc in [('nclx-p3',12,13),('nclx-709',1,1),('pq',9,16),('hlg',9,18)]:
    props=p3base+[box(b'colr',b'nclx'+u16(cp)+u16(tc)+u16(6)+b'\x80')]
    write(name,[p3image],props,{1:list(range(1,len(props)+1))},1,[])
# EXIF only, then a deliberate conflict between EXIF 90 degrees and HEIF 180.
exif=b'\0\0\0\x06Exif\0\0MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0\x06\0\0\0\0\0\0'
for name,rotation in [('exif-only',None),('conflicting-exif',2)]:
    props=base+([] if rotation is None else [box(b'irot',bytes([rotation]))])
    write(name,[image,(2,b'Exif',False,exif)],props,{1:list(range(1,len(props)+1))},1,[(b'cdsc',2,[1])])
write('tone-map-item',[image,(2,b'tmap',True,b'\0')],base,{1:[1,2,3]},1,[])
# Grid tiles intentionally reuse one owned compressed image. Mark it hidden.
grid=bytes([0,0,1,1])+u16(256)+u16(192)
props=base+[full(b'ispe',u32(256)+u32(192))]
write('grid',[(1,b'hvc1',True,image[3]),(2,b'grid',False,grid)],props,{1:[1,2,3],2:[4]},2,[(b'dimg',2,[1,1,1,1])])
# Preserve the alpha auxiliary's actual HEVC payload/properties while changing
# its role, for metadata-based gain/depth policy checks before any decode.
_,_,alpha_props,alpha_map,alpha_payloads,alpha_primary=source('alpha.heic')
auxiliary=next(item for item,values in alpha_map.items() if item!=alpha_primary and any(alpha_props[v-1][0]==b'auxC' for v in values))
for name,role in [('apple-gain','urn:com:apple:photo:2020:aux:hdrgainmap'),('iso-gain','urn:iso:std:iso:ts:21496:-1'),('depth','urn:mpeg:hevc:2015:auxid:2')]:
    props=[];associations={};
    for new_id,old_id in [(1,alpha_primary),(2,auxiliary)]:
        values=[]
        for index in alpha_map[old_id]:
            kind,raw,payload=alpha_props[index-1]
            if kind==b'auxC':raw=full(b'auxC',role.encode()+b'\0')
            props.append(raw);values.append(len(props))
        associations[new_id]=values
    write(name,[(1,b'hvc1',False,alpha_payloads[alpha_primary]),(2,b'hvc1',True,alpha_payloads[auxiliary])],props,associations,1,[(b'auxl',2,[1])])
print(args.output)
