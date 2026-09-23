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


def unicode_fixture(path, invalid=False, supplementary=b"D83DDE00"):
    """Synthetic ToUnicode mappings; glyph shape/font coverage is not asserted."""
    cmap = b'/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Fileform) /Ordering (Test) /Supplement 0 >> def\n/CMapName /FileformTest def\n/CMapType 2 def\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n4 beginbfchar\n<41> <03A9>\n<42> <4E2D>\n<43> <D83DDE00>\n<44> <0301>\nendbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n'
    cmap = cmap.replace(b"D83DDE00", supplementary)
    if invalid:
        cmap = cmap.replace(b'<03A9>', b'<0000>')
    content = b'BT /F1 12 Tf 20 100 Td (ABCD) Tj ET\n'
    objects = [
        b'<< /Type /Catalog /Pages 2 0 R >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        b'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>',
        f'<< /Length {len(content)} >>\nstream\n'.encode()+content+b'endstream',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 6 0 R >>',
        f'<< /Length {len(cmap)} >>\nstream\n'.encode()+cmap+b'endstream',
    ]
    write_pdf(path, objects)


def annotation_fixture(path, rotation=0, reveal_hidden=False, generated_widget=False, xfa=False):
    def appearance(text, color):
        data=f'q {color} rg 0 0 140 30 re f Q BT /Helv 14 Tf 0 g 5 8 Td ({text}) Tj ET\n'.encode()
        return f'<< /Type /XObject /Subtype /Form /BBox [0 0 140 30] /Resources << /Font << /Helv 5 0 R >> >> /Length {len(data)} >>\nstream\n'.encode()+data+b'endstream'
    widget_ap=b'' if generated_widget else b'/AP << /N 8 0 R >> '
    objects=[
        b'<< /Type /Catalog /Pages 2 0 R /AcroForm 7 0 R /OpenAction << /S /JavaScript /JS (this.getField("visible").value = "CHANGED";) >> >>',
        b'<< /Type /Pages /Count 1 /Kids [3 0 R] >>',
        f'<< /Type /Page /Parent 2 0 R /MediaBox [10 20 210 320] /CropBox [20 30 200 300] /Rotate {rotation} /Resources << >> /Contents 4 0 R /Annots [6 0 R 9 0 R 11 0 R 13 0 R] >>'.encode(),
        b'<< /Length 0 >>\nstream\nendstream',
        b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
        b'<< /Type /Annot /Subtype /Widget /FT /Tx /T (visible) /V (Visible form) /Rect [40 190 180 220] /F 4 /DA (/Helv 14 Tf 0 g) /MK << /BG [1 1 0] >> '+widget_ap+b'/P 3 0 R >>',
        b'<< /Fields [6 0 R 9 0 R] /DA (/Helv 14 Tf 0 g) /DR << /Font << /Helv 5 0 R >> >> '+(b'/XFA [] ' if xfa else b'')+b'>>',
        appearance('Visible form','1 1 0'),
        f'<< /Type /Annot /Subtype /Widget /FT /Tx /T (hidden) /V (Hidden form) /Rect [40 50 180 80] /F {4 if reveal_hidden else 36} /AP << /N 10 0 R >> /P 3 0 R >>'.encode(),
        appearance('Hidden form','1 0 1'),
        b'<< /Type /Annot /Subtype /FreeText /Contents (Visible note) /Rect [40 120 180 150] /F 4 /DA (/Helv 14 Tf 0 g) /AP << /N 12 0 R >> /P 3 0 R >>',
        appearance('Visible note','0 1 1'),
        f'<< /Type /Annot /Subtype /FreeText /Contents (Hidden note) /Rect [40 85 180 115] /F {4 if reveal_hidden else 6} /AP << /N 14 0 R >> /P 3 0 R >>'.encode(),
        appearance('Hidden note','1 0.5 0'),
    ]
    write_pdf(path,objects)
