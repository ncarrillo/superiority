function bitWidthForRange(minimum: number, maximum: number): number {
  const cardinality = maximum - minimum + 1;
  return Math.max(1, Math.ceil(Math.log2(cardinality)));
}

function parseRange(comment: string): { minimum: string; maximum: string } | undefined {
  const match = comment.match(/(-?[\d,]+)\.\.=(-?[\d,]+)/);
  if (!match) return undefined;
  return {
    minimum: match[1].replaceAll(",", ""),
    maximum: match[2].replaceAll(",", ""),
  };
}

function parseCountBits(comment: string): number | undefined {
  const explicit = comment.match(/(?:count:\s*)?(\d+)(?:-bit|\s+(?:wire\s+)?bits?)(?:\s+(?:element|byte))?\s*count/i)
    ?? comment.match(/count:\s*(\d+)\s+(?:wire\s+)?bits?/i)
    ?? comment.match(/(\d+)-bit\s+(?:element|byte)\s+count/i);
  if (explicit) return Number.parseInt(explicit[1], 10);
  if (/byte count:\s*u8\b|\bu8\s+byte\s+count\b/i.test(comment)) return 8;

  const range = parseRange(comment);
  if (!range) return undefined;
  return bitWidthForRange(Number(range.minimum), Number(range.maximum));
}

function containerProperties(kind: "Array" | "Blob" | "Utf8", comment: string): string[] {
  const properties: string[] = [];
  const bits = parseCountBits(comment);
  const range = parseRange(comment);
  const maximum = comment.match(/maximum\s+([\d,]+)/i)?.[1]?.replaceAll(",", "");
  const exactCount = comment.match(/exactly\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+elements/i)?.[1];
  const prefix = kind === "Array" ? "count" : "byte_count";

  if (bits !== undefined) properties.push(`${prefix}_bits: ${bits}`);
  if (range) properties.push(`${prefix}_range: ${range.minimum}..=${range.maximum}`);
  else if (maximum) properties.push(`${prefix}_range: 0..=${maximum}`);
  if (exactCount) properties.push(`${prefix}: ${exactCount}`);
  if (/stored as len\s*-\s*2/i.test(comment)) {
    properties.push("byte_count_encoding: length_minus_2");
  }
  if (kind === "Blob" || kind === "Utf8") properties.push("alignment: byte");
  return properties;
}

function normalizeTypeExpression(expression: string): string {
  let result = expression;
  result = result.replace(/\bBits<(\d+)>/g, "u$1");
  result = result.replace(/\[u8;\s*(\d+)\]/g, "Bytes<$1>");
  result = result.replace(/Vec<u8>/g, "Blob");
  result = result.replace(/Vec<([^<>]+)>/g, "Array<$1>");
  result = result.replace(/Option<(.+)>/g, "Optional<$1>");
  result = result.replace(/Vec<([^<>]+)>/g, "Array<$1>");
  result = result.replace(/\bString\b/g, "Utf8");
  result = result.replace(/\s*\{\s*value:\s*([^,{}]+?)\s*,?\s*\}\s*$/, " = $1");
  return result;
}

export function normalizeSunkenSchema(source: string): string {
  const lines = source.trim().split("\n");
  const output: string[] = [];

  for (const line of lines) {
    const field = line.match(/^(\s*)([a-zA-Z_][a-zA-Z0-9_]*):\s*(.+?),(?:\s*\/\/\s*(.*))?$/);
    if (!field) {
      output.push(normalizeTypeExpression(line));
      continue;
    }

    const [, indent, name, originalType, comment = ""] = field;
    const type = normalizeTypeExpression(originalType);
    const array = type.match(/^Array<(.+)>$/);
    const selectorBits = comment.match(/selector:\s*(\d+)\s+(?:wire\s+)?bits?/i)?.[1];
    const kind = array
      ? "Array"
      : type === "Blob"
        ? "Blob"
        : type === "Utf8" && /byte count/i.test(comment)
          ? "Utf8"
          : undefined;
    const properties = kind ? containerProperties(kind, comment) : [];

    if (selectorBits) {
      output.push(`${indent}${name}: ${type} {`);
      output.push(`${indent}    selector_bits: ${selectorBits},`);
      output.push(`${indent}},`);
      continue;
    }

    if (!kind || properties.length === 0) {
      output.push(`${indent}${name}: ${type},${comment ? ` // ${comment}` : ""}`);
      continue;
    }

    output.push(`${indent}${name}: ${type} {`);
    for (const property of properties) output.push(`${indent}    ${property},`);
    output.push(`${indent}},`);
  }

  return output.join("\n");
}
