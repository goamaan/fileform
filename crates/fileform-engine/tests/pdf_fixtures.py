"""Generated PDF fixtures; no user documents or production dependency."""
def fixture(path, annotated=False, inherited=False, text=False):
    objects=[b'<< /Type /Catalog /Pages 2 0 R >>',b'<< /Type /Pages /Count 2 /Kids [3 0 R 4 0 R] >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 5 0 R >>',b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Resources << >> /Contents 6 0 R >>']
    if inherited:
        objects[1]=objects[1].replace(b' >>',b' /MediaBox [-10 -20 210 320] /CropBox [0 0 200 300] /Rotate -90 >>')
        objects[2]=objects[2].replace(b'/MediaBox [0 0 200 300] ',b'/BleedBox [1 2 100 200] ')
        objects[3]=objects[3].replace(b'/MediaBox [0 0 300 200] ',b'/TrimBox null ')
    if annotated: objects[2]=objects[2].replace(b'/Resources',b'/Annots [] /Resources')
    contents=[b'q 1 0 0 rg 10 10 50 50 re f Q\n',b'q 0 0 1 rg 20 20 40 40 re f Q\n']
    if text:
        objects[2]=objects[2].replace(b'/Resources << >>',b'/Resources << /Font << /F1 7 0 R >> >>')
        objects[3]=objects[3].replace(b'/Resources << >>',b'/Resources << /Font << /F1 7 0 R >> >>')
        contents=[content+f'BT /F1 12 Tf 20 100 Td (Fileform page {i+1}) Tj ET\n'.encode() for i,content in enumerate(contents)]
    for content in contents:
        objects.append(f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream')
    if text: objects.append(b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
    write_pdf(path, objects)

def write_pdf(path, objects):
    data=bytearray(b'%PDF-1.7\n');offsets=[0]
    for index,obj in enumerate(objects,1):
        offsets.append(len(data));data.extend(str(index).encode()+b' 0 obj\n'+obj+b'\nendobj\n')
    xref=len(data);data.extend(f'xref\n0 {len(objects)+1}\n0000000000 65535 f \n'.encode())
    for offset in offsets[1:]:data.extend(f'{offset:010} 00000 n \n'.encode())
    data.extend(f'trailer\n<< /Size {len(objects)+1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n'.encode());path.write_bytes(data)


def visual_fixture(path, rotation=0, user_unit=1):
    """Negative-origin cropped page, RGB image and half-opacity vector overlay."""
    pixels = bytes([255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255])
    content = b'q 0 1 1 rg -8 -18 6 6 re f Q\nq 40 0 0 40 0 0 cm /Im1 Do Q\nq /GS1 gs 1 1 0 rg 0 0 20 20 re f Q\n'
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        f'<< /Type /Page /Parent 2 0 R /MediaBox [-10 -20 70 80] /CropBox [0 0 60 60] /Rotate {rotation} /UserUnit {user_unit} /Resources << /XObject << /Im1 5 0 R >> /ExtGState << /GS1 6 0 R >> >> /Contents 4 0 R >>'.encode(),
        f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream',
        f'<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate false /Length {len(pixels)} >>\nstream\n'.encode()+pixels+b'\nendstream',
        b'<< /Type /ExtGState /ca 0.5 /CA 0.5 >>',
    ]
    write_pdf(path, objects)
