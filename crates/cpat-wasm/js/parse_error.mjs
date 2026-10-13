// v12's `ParseError`: where a pattern failed. `lastIndex` is inclusive (how
// far the attempt got), so `endIndex` is one past it.
export class ParseError {
  constructor(startIndex, lastIndex, pattern) {
    this.firstIndex = startIndex;
    this.startIndex = startIndex;
    this.lastIndex = lastIndex;
    this.endIndex = lastIndex + 1;
    this.pattern = pattern;
  }
}
