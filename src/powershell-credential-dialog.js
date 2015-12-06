'use strict';
const childProcess = require('child_process');
const ps = s => new Promise((y, n) => 
  childProcess.exec(
    'powershell /c ' + s,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim())
    }
  )
);
module.exports = (username, message) =>
  ps(
    `$c = Get-Credential -Message \\"${message}\\" -Username ${username}; ` +
    `$c.GetNetworkCredential().username; echo \\"|||||\\"; ` +
    `$c.GetNetworkCredential().password;`
  )
  .then(r => {
    let answers = r.split('|||||').map(s => s.trim());
    return {
      username: answers[0],
      password: answers[1]
    };
  });
