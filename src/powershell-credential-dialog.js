'use strict';
const runPowershell = require('./run-powershell');
const delimiter = '|||||';
module.exports = (service, username, message) => {
  message = message ||
    `Please enter your username and password to login to ${service}.`;
  return runPowershell(
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
