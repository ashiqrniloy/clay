export function encodeUtf8(text) {
  if (typeof text !== "string") throw new Error("lsp.invalid_utf8: string required");
  const bytes = [];
  for (let index = 0; index < text.length; index += 1) {
    const code = text.codePointAt(index);
    if (code >= 0xd800 && code <= 0xdfff) throw new Error("lsp.invalid_utf8: unpaired surrogate");
    if (code > 0xffff) index += 1;
    if (code <= 0x7f) bytes.push(code);
    else if (code <= 0x7ff) bytes.push(0xc0 | code >> 6, 0x80 | code & 0x3f);
    else if (code <= 0xffff) bytes.push(0xe0 | code >> 12, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
    else bytes.push(0xf0 | code >> 18, 0x80 | code >> 12 & 0x3f, 0x80 | code >> 6 & 0x3f, 0x80 | code & 0x3f);
  }
  return Uint8Array.from(bytes);
}

export function decodeUtf8(bytes) {
  if (!(bytes instanceof Uint8Array)) throw new Error("lsp.invalid_utf8: Uint8Array required");
  const points = [];
  for (let index = 0; index < bytes.length;) {
    const first = bytes[index++];
    if (first <= 0x7f) {
      points.push(first);
      continue;
    }
    const length = first >= 0xc2 && first <= 0xdf ? 2
      : first >= 0xe0 && first <= 0xef ? 3
      : first >= 0xf0 && first <= 0xf4 ? 4
      : 0;
    if (length === 0 || index + length - 1 > bytes.length) throw new Error("lsp.invalid_utf8: malformed sequence");
    let code = first & (0x7f >> length);
    for (let offset = 1; offset < length; offset += 1) {
      const next = bytes[index++];
      if ((next & 0xc0) !== 0x80) throw new Error("lsp.invalid_utf8: malformed continuation");
      code = code << 6 | next & 0x3f;
    }
    if ((length === 3 && code < 0x800) || (length === 4 && code < 0x10000)
        || (code >= 0xd800 && code <= 0xdfff) || code > 0x10ffff) {
      throw new Error("lsp.invalid_utf8: overlong or invalid scalar");
    }
    points.push(code);
  }
  let text = "";
  for (let index = 0; index < points.length; index += 4096) {
    text += String.fromCodePoint(...points.slice(index, index + 4096));
  }
  return text;
}

export function utf8ByteLength(text) {
  return encodeUtf8(text).length;
}
