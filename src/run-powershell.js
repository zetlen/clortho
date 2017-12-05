'use strict';
const childProcess = require('child_process');
module.exports = (s, isFile) => new Promise((resolve, reject) =>
  childProcess.exec(
    'powershell -' + (isFile ? 'File' : 'Command') + ' ' + s,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return reject(e);
      if (stderr) return reject(new Error(stderr));
      resolve(stdout.trim());
    }
  )
);
