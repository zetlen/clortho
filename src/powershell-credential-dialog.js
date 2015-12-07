'use strict';
const childProcess = require('child_process');
const ps = s => new Promise((y, n) =>
  childProcess.exec(
    'powershell /c ' + s,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return n(e);
      if (stderr) return n(new Error(stderr));
      y(stdout.trim());
    }
  )
);
const delimiter = '|||||';
module.exports = (service, username, message) => {
  message = message || `Please enter your username and password to login to ${service}.`;
  return ps(
    `$c = Get-Credential -Message \\"${message}\\" -Username ${username}; ` +
    `$c.GetNetworkCredential().username; echo \\"${delimiter}\\"; ` +
    `$c.GetNetworkCredential().password;`
  )
  .then(r => {
    let answers = r.split(delimiter).map(s => s.trim());
    return {
      username: answers[0],
      password: answers[1]
    };
  });
};
