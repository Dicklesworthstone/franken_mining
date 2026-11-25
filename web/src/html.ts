/** Text helpers that both the build and the browser bundle use. Nothing here touches the DOM or the process. */

export const escape = (s: string) =>
  s.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;").replaceAll('"', "&quot;");

const NBSP = " ";
const WJ = "⁠";
const UNITS =
  "t|kt|Mt|Gt|kg|g|g/t|oz|koz|Moz|lb|m|km|mm|cm|µm|ha|s|ms|h|t/h|kWh|kWh/t|MW|°|%|days?|weeks?|months?|years?|blocks?|holes?|samples?|trucks?|ounces?|tonnes?|metres?|million|billion|bn|M";
const MONTHS = "January|February|March|April|May|June|July|August|September|October|November|December";

/**
 * Keeps a figure whole at a line break: a no-break space between a number and its unit, inside a date and after a
 * section sign, and a word joiner after the dash of a range ("$30–57M"). Works on text only, never inside a tag.
 */
export function keep(html: string) {
  return html.replace(/(^|>)([^<]+)/g, (_, open: string, text: string) => {
    const kept = text
      .replace(new RegExp(`(\\d[\\d,.]*) (?=(?:${UNITS})\\b)`, "g"), `$1${NBSP}`)
      .replace(new RegExp(`\\b(\\d{1,2}) (?=(?:${MONTHS})\\b)`, "g"), `$1${NBSP}`)
      .replace(new RegExp(`\\b(${MONTHS}) (?=\\d{4}\\b)`, "g"), `$1${NBSP}`)
      .replace(/\b(Q[1-4]) (?=\d{4}\b)/g, `$1${NBSP}`)
      .replace(/(\d)–(?=[$\d])/g, `$1–${WJ}`)
      .replace(/(§|\$|US\$) (?=\d)/g, `$1${NBSP}`)
      .replace(/ (?=· \d)/g, NBSP);
    return open + kept;
  });
}
