# Generated image fixtures

`progressive-gray.jpg` is an original 8 × 4 uniform grayscale fixture generated
with Pillow 11.3.0:

```python
from PIL import Image
Image.new('L', (8, 4), 128).save('progressive-gray.jpg', quality=95, progressive=True)
```

It contains no personal metadata or third-party image content. It is retained so
the same progressive JPEG test runs on macOS and Windows without a Python codec
dependency at test time. Project licensing applies to the fixture.
