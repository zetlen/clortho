'use strict';
const childProcess = require('child_process');
const osa = (service, username, message) => new Promise((resolve, reject) => {
  message = message || `Please enter your password for ${username} to login to ${service}.`;
  childProcess.exec(
    `osascript -e 'tell app "System Events" to return display dialog "${message}" with title "Password for ${username} at ${service}" with icon caution default answer "" buttons { "Cancel", "OK" } default button 2 with hidden answer'`,
    { stdio: 'ignore' },
    (e, stdout, stderr) => {
      if (e) return reject(e);
      if (stderr) return reject(new Error(stderr));
      resolve(stdout.trim());
    }
  );
});
let requirePassword = (service, username, message, repeated) =>
  osa(service, username, message).then(r => {
    let m = r.match(/text returned:(.*)/);
    if (!m || !m[1]) {
      return requirePassword(service, username, !repeated ? 'A password is required.\n\n' + message : message, true);
    }
    return m[1];
  });
module.exports = (service, username, message) =>
  requirePassword(service, username, message).then(p => ({
    username: username,
    password: p
  }));
