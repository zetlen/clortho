'use strict';
const codeRe = /clortho: ([A-Z_]+):/;
module.exports = {
  create (code, message) {
    return new Error(`clortho: ${code}: ${message}`);
  },
  getCode (err) {
    const m = err.message.match(codeRe);
    return m && m[1];
  }
};
