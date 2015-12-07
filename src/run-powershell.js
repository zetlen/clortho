'use strict';
const childProcess = require('child_process');
module.exports = (s, isFile) => new Promise((y, n) =>
  childProcess.exec(
    'powershell -' + (isFile ? 'File' : 'Command') + ' ' + s,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim());
    }
  )
);
