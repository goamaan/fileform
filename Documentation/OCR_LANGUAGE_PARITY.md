# Automatic OCR language parity

Research snapshot: 2026-10-01. This documents a route and measured model hashes,
not completed multilingual recognition or accuracy equivalence.

## Original behavior and current gap

[DocumentBackend.swift](../Sources/FileformCore/DocumentBackend.swift) creates a
`VNRecognizeTextRequest` with accurate recognition, language correction, and
`automaticallyDetectsLanguage=true`. It does not set recognition languages or
request revision. Apple describes automatic detection as an *attempt* to choose
the appropriate recognition/correction model. Supported languages depend on the
request/revision; no public source establishes a universal fixed language list or
perfect automatic detection. [Automatic detection](https://developer.apple.com/documentation/vision/vnrecognizetextrequest/automaticallydetectslanguage),
[supported languages](https://developer.apple.com/documentation/vision/vnrecognizetextrequest/supportedrecognitionlanguages()),
[language ordering](https://developer.apple.com/documentation/vision/vnrecognizetextrequest/recognitionlanguages).

A read-only local Swift probe using those exact request settings returned:

```text
macOS 26.2 (25C56), request revision 3
recognitionLanguages: [en_US]
supportedRecognitionLanguages:
en-US fr-FR it-IT de-DE es-ES pt-BR zh-Hans zh-Hant yue-Hans yue-Hant
ko-KR ja-JP ru-RU uk-UA th-TH vi-VT ar-SA ars-SA tr-TR id-ID cs-CZ
da-DK nl-NL no-NO nn-NO nb-NO ms-MY pl-PL ro-RO sv-SE
```

The default `en_US` does **not** prove English-only behavior when auto detection is
on. Conversely, a supported identifier does not prove it will always be selected
automatically. Preserve this observed 30-identifier scope as the comparison matrix,
probe the original on fixtures, and record OS/revision with results. Older supported
macOS versions may differ. There is no public Apple implementation to duplicate.

Current [ocr_image.rs](../crates/fileform-engine/src/ocr_image.rs) only accepts `Eng`,
copies English into its owned workspace and runs `--oem 1 --psm 3 -l eng` (with a
second thresholding attempt only if empty). [ocr_pack.rs](../crates/fileform-engine/src/ocr_pack.rs)
truthfully reports automatic detection false. `osd` is verified in the pack but is
not used by this recognition path. Therefore bundled OSD is not restored language
or orientation parity.

## Portable approach and honest boundaries

Tesseract accepts ordered multiple-language sets and script models; both timing
and results depend on model ordering. `--psm 3` is layout analysis without OSD.
[Official command guide](https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html).
OSD produces orientation and script, **not language identification**. Its source
skips insufficient-character inputs; it cannot reliably select a model from every
short label or mixed-script page. [OSD source](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/src/ccmain/osdetect.cpp),
[API implementation](https://github.com/tesseract-ocr/tesseract/blob/5.5.3/src/api/baseapi.cpp).

Recommended staged implementation (design proposal, requires experiments):

1. Run bounded OSD on an owned raster with legacy/OSD support available. Record
   script/orientation confidences and failure separately from OCR failure. Rotate
   an owned raster once; don't apply OSD orientation twice via an OSD-enabled PSM.
2. Recognize with a multilingual script model for the observed script. Latin
   script recognition can recover several languages without claiming to identify
   which language each word belongs to. Cyrillic needs explicit English alongside
   it when mixed. Do not map every Han result to simplified Chinese: Han can mean
   traditional Chinese, Japanese or Cantonese. Use bounded competing candidates,
   or a validated combined model set, for that ambiguity.
3. For short text/weak OSD, use bounded fallback candidates and assess recognized
   script consistency, coverage and character/word confidence. Tesseract confidence
   is **not a calibrated cross-language probability**; largest mean confidence alone
   is not a justified winner. Mixed scripts need region-level recognition or a
   measured combined-model strategy, not a single whole-page winner.
4. If useful, a separate offline text-language identifier can refine same-script
   choices after a first pass; no such dependency is selected by this research.
   It cannot rescue characters the first model never recognized and would require
   its own licenses, model pins and evaluation. Explicit language override remains
   an escape hatch, not a substitute for the required default automatic flow.
5. Keep a single per-page/document deadline across OSD, candidate trials and retries,
   cap resident model count, and report the actual attempted/selected models in
   receipts. Label the product behavior “automatic multilingual OCR” only after
   testing, and reserve language-detection claims for actual measured detection.

A bounded default distribution can contain the language models below (roughly
80 MB including OSD/vertical variants) while loading only relevant models per
attempt. This covers candidate models corresponding to the observed Vision list,
**not equivalent recognition quality or exact dialect support**: `nor` does not
prove separate Bokmål/Nynorsk parity, `ara` does not establish Najdi Arabic (`ars`),
and `chi_sim`/`chi_tra` do not prove Cantonese (`yue`). Test these explicitly and
leave gaps open. Single-script strategy alternatives include pinned `script/Latin`
(89,384,811 bytes) and `script/Cyrillic` (29,252,466 bytes); their memory costs can
exceed a small model cap. Do not load all language/script models simultaneously.
[Fast model scope and script exclusions](https://github.com/tesseract-ocr/tessdata_fast/blob/87416418657359cb625c412a48b6e1d6d41c29bd/README.md).

## Exact model pins

All assets below were streamed from the immutable revision already used by
[build-ocr-evaluation.py](../Tools/build-ocr-evaluation.py):
`87416418657359cb625c412a48b6e1d6d41c29bd` of `tesseract-ocr/tessdata_fast`.
SHA-256 was computed over asset bytes (not Git blob IDs); no user data was read,
models were not installed, and no implementation files were changed.
URL template: `https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/87416418657359cb625c412a48b6e1d6d41c29bd/{name}.traineddata`.
[Immutable upstream tree](https://github.com/tesseract-ocr/tessdata_fast/tree/87416418657359cb625c412a48b6e1d6d41c29bd).

| Name | Bytes | SHA-256 |
| --- | ---: | --- |
| eng | 4113088 | `7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2` |
| osd | 10562727 | `9cf5d576fcc47564f11265841e5ca839001e7e6f38ff7f7aacf46d15a96b00ff` |
| fra | 1130365 | `ced037562e8c80c13122dece28dd477d399af80911a28791a66a63ac1e3445ca` |
| ita | 2701314 | `b8f89e1e785118dac4d51ae042c029a64edb5c3ee42ef73027a6d412748d8827` |
| deu | 1525436 | `19d219bbb6672c869d20a9636c6816a81eb9a71796cb93ebe0cb1530e2cdb22d` |
| spa | 2294433 | `6f2e04d02774a18f01bed44b1111f2cd7f3ba7ac9dc4373cd3f898a40ea6b464` |
| por | 1982756 | `c4932b937207a9514b7514d518b931a99938c02a28a5a5a553f8599ed58b7deb` |
| chi_sim | 2469156 | `a5fcb6f0db1e1d6d8522f39db4e848f05984669172e584e8d76b6b3141e1f730` |
| chi_tra | 2366642 | `529c5b5797d64b126065cd55f2bb4c7fd7b15790798091b1ff259941a829330b` |
| kor | 1677415 | `6b85e11d9bbf07863b97b3523b1b112844c43e713df8b66418a081fd1060b3b2` |
| jpn | 2471260 | `1f5de9236d2e85f5fdf4b3c500f2d4926f8d9449f28f5394472d9e8d83b91b4d` |
| rus | 3861738 | `e16e5e036cce1d9ec2b00063cf8b54472625b9e14d893a169e2b0dedeb4df225` |
| ukr | 3825102 | `d59e53e2bded32f4445f124b4b00240fcac7e8044c003ab822ccb94f0b3db59b` |
| tha | 1072600 | `294227cc2d1292b0acb28d61d4115c88252b96d466ca90b417cf4cf0c67bf07c` |
| vie | 531275 | `79df64caf7bcfb2a27df5042ecb6121e196eada34da774956995747636d5bfa1` |
| ara | 1432056 | `e3206d3dc87fd50c24a0fb9f01838615911d25168f4e64415244b67d2bb3e729` |
| tur | 4550554 | `7393381111e1152420fc4092cb44eef4237580d21b92bf30d7d221aad192c6b7` |
| ind | 1122661 | `69786901da87ab8766c1ea7fbb10b28f2110c14da3f6c8f2735df131fba95d88` |
| ces | 3795684 | `934bcaf97ef3348413263331131c9fa7f55f30db333c711929c124fb635f7e1b` |
| dan | 2580059 | `acb1fd074487a31d1294fcdfd7d7c673467ffd8aeacb2ccd61ebcbf04eb4e2fa` |
| nld | 6050296 | `ced0e5e046a84c908a6aa7accbef9a232c4a5d9a8276691b81c6ee64d02963f6` |
| nor | 3610079 | `0451eb4f8049ae78196806bf878a389a2f40f1386fe038568cf4441226ba6ef2` |
| msa | 1747801 | `e41a3e5febfec50c90371eb1cbb17a48b10cad387900e3420b1f134c1b766cba` |
| pol | 4765518 | `c4476cdbc0e33d898d32345122b7be1cbf85ace15f920f06c7714756e1ef79b2` |
| ron | 2376323 | `9adfde6b51ba4b97efd10ea37c3070fd3fc2bad7815e81f5c3c198cd96216cc9` |
| swe | 4167034 | `f7304988d41f833efebcc2d529df54b1903ecebbc3da1faabd19a0fddd4fe586` |
| jpn_vert | 3037480 | `bf1e2640954691797e2dc14f38533e601b59ee37958698ae0f0b81dc6f09c71b` |
| chi_sim_vert | 1927902 | `20590de84725bab69cde93bd6e8ed360a13cc5421a7e7364ddeb93e9af53d6da` |
| chi_tra_vert | 1824756 | `1df02a4b210e5c217b783819538b63e9dfe6904e2b5e53b62664f1b9f7a989d0` |
| script/Latin | 89384811 | `6dbdaf8ecc6c40f025c2648bf3b3f3fbffe073e1fd2df2047fde2e2b2f020d53` |
| script/Cyrillic | 29252466 | `a80325ebb1c7aa2dca5002ec05b15052a51dcb2e9e65373c264a3df7ac284358` |
| script/Arabic | 9311038 | `47c262ac4e843c024df87ffa1363b77a821a843c069c71ef7650f4d2ecfea1e8` |
| script/HanS | 5972903 | `d03c250faaa48015bfde2213f3d7d5bda82bcee93422fceac5906a32073d9152` |
| script/HanT | 5448070 | `29588327f0d830b5c112209045d826b9be014138266b20f2b08843aac2730fbb` |
| script/Japanese | 5877645 | `c176cdca0c17e9be97bda854ac91baea90b92e2813b45749ad05e409477ba8b2` |
| script/Hangul | 4845849 | `cae2d12b8cdac6b62643b65915bec347e80970adc55228ca42ac1ec8909b198a` |
| script/Thai | 4249016 | `d9e34be94556ddb65fd7e04892ff606e1a5b696d0eb422b5a7efc6859943702d` |
| script/Vietnamese | 1667794 | `30546eea618459e79761f962391e6879778b026ad7b4459a11664a1b654758bd` |

`script/Korean` does not exist at this revision; use `script/Hangul`. Fast models
use LSTM; retain legacy capability for OSD. Model data are Apache-2.0; ship the
exact upstream license. Inspect traineddata configs for implicit secondary-model
loads (the README describes Japanese vertical loading); fail clearly on missing
assets rather than silently changing coverage. [Pinned README/license](https://github.com/tesseract-ocr/tessdata_fast/tree/87416418657359cb625c412a48b6e1d6d41c29bd).

## Reliable locally generated acceptance fixtures

Use original authored strings and deterministic local raster generation, not user
documents or downloaded screenshots. Mac CoreText can shape Arabic/Thai and CJK
correctly; its installed Geeza Pro, Thonburi, Hiragino and Apple SD Gothic fonts
were confirmed present. For reproducible cross-platform regeneration, pin suitable
redistributable fonts and shaping dependencies separately. Do not redistribute
Apple font files. [CoreText line creation](https://developer.apple.com/documentation/coretext/ctlinecreatewithattributedstring(_:)).

Generate and visually inspect each base raster once; preserve its PNG hash and
UTF-8 ground truth. Ensure no missing-glyph boxes and correct Arabic shaping/
right-to-left order before testing OCR. Build white-background paragraphs with
at least several lines and >100 letters for OSD; also test one-line labels where
OSD legitimately fails. Include original text with French/German accents, Turkish
İ/ı, Vietnamese tone marks, Russian/Ukrainian distinctions, Arabic joining,
Thai segmentation, simplified/traditional Chinese, Japanese kana+kanji and Korean.
Include `Invoice 4827` in bilingual cases to expose loss of Latin/numeric content.

Rotate each raster 0/90/180/270 degrees; distinguish rotation from vertical CJK
writing (both need tests). Add mixed-script lines and pages, two columns, sparse
receipts, blur/low contrast, punctuation, digits, blank pages and symbols-only
inputs. Run identical images through original Vision default-auto and portable
auto; use explicit-language runs diagnostically, not as replacement acceptance.
Measure normalized Unicode character error rate and critical number accuracy,
orientation, ordering, elapsed time and peak memory. Preserve combining marks and
script distinctions; don't normalize away the very characters being evaluated.

No fixture images were generated or OCR accuracy measured in this research task.
These experiments, all observed language/dialect rows, mixed-script handling,
short-text fallback, Windows execution, bounded cancellation and receipt semantics
remain open. Neither English+OSD nor a small script subset closes full parity.
