"""Independent PNG decoder for generated test outputs, including all five filters."""
import struct
import zlib
def decode(path, rgba=False):
    data=path.read_bytes();assert data[:8]==b'\x89PNG\r\n\x1a\n'
    offset=8;chunks={};compressed=bytearray()
    while offset<len(data):
        length,=struct.unpack('>I',data[offset:offset+4]);kind=data[offset+4:offset+8]
        value=data[offset+8:offset+8+length]
        crc,=struct.unpack('>I',data[offset+8+length:offset+12+length])
        assert zlib.crc32(kind+value)==crc
        offset+=12+length
        if kind==b'IDAT':compressed.extend(value)
        else:chunks[kind]=value
        if kind==b'IEND':assert offset==len(data);break
    w,h,depth,color,compression,filtering,interlaced=struct.unpack('>IIBBBBB',chunks[b'IHDR'])
    assert (depth,compression,filtering,interlaced)==(8,0,0,0) and color in (2,6)
    channels=3 if color==2 else 4
    scan=zlib.decompress(compressed);stride=w*channels
    assert len(scan)==h*(stride+1)
    previous=bytearray(stride);pixels=bytearray()
    for y in range(h):
        start=y*(stride+1);kind=scan[start];row=bytearray(scan[start+1:start+1+stride])
        assert kind<=4
        if kind==2:row=bytearray((a+b)&255 for a,b in zip(row,previous))
        elif kind:
            for x in range(stride):
                left=row[x-channels] if x>=channels else 0;up=previous[x];corner=previous[x-channels] if x>=channels else 0
                if kind==1:prediction=left
                elif kind==3:prediction=(left+up)//2
                else:
                    p=left+up-corner;a=abs(p-left);b=abs(p-up);c=abs(p-corner)
                    prediction=left if a<=b and a<=c else (up if b<=c else corner)
                row[x]=(row[x]+prediction)&255
        pixels.extend(row);previous=row
    if channels==4 and not rgba:
        assert all(alpha==255 for alpha in pixels[3::4])
        pixels=bytearray(value for i,value in enumerate(pixels) if i%4!=3)
    assert b'sRGB' in chunks or b'iCCP' in chunks
    if b'sRGB' in chunks: assert chunks[b'sRGB']==b'\0'
    return w,h,struct.unpack('>IIB',chunks[b'pHYs']) if b'pHYs' in chunks else None,pixels
