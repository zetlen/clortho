const path = require('path');
const jsStringEscape = require('js-string-escape');
const runPowershell = require('./run-powershell');
const ErrorManager = require('./error-manager');
const credManPath = path.resolve(__dirname, '../CredMan.ps1');
const runCredMan = (cmd, opts) =>
  runPowershell(
    Object.keys(opts).reduce(
      (cmd, k) => cmd + ` -${k} '${jsStringEscape(opts[k])}'`,
      `"${credManPath}" -${cmd}`
    ),
    true
  );
const passwordLineRE = /^[\s\t]*Password[\s\t]*:[\s\t]?'(.*)'/;
const createTargetName = (service, account) => `${service};user=${account}`;

module.exports = {
  get: (service, account) => runCredMan(
    'GetCred',
    {
      Target: createTargetName(service, account)
    }
  ).then(res => {
    if (res.match(/was not found\.$/)) {
      throw ErrorManager.create(
        'GET_FAILURE',
        `Could not find ${service} password for ${account}`
      );
    }
    let pwl = res.split('\n').find(l => !!l.match(passwordLineRE));
    if (!pwl) {
      throw ErrorManager.create(
        'GET_FAILURE',
        `Unknown error finding ${service} password for ${account}.`
      );
    }
    return { username: account, password: pwl.match(passwordLineRE)[1] };
  }),
  set: (service, account, password) => runCredMan(
    'AddCred',
    {
      Target: createTargetName(service, account),
      User: account,
      Pass: password
    }
  ).then(res => {
    if (res.indexOf('Successfully') !== 0) {
      throw ErrorManager.create(
        'KEYCHAIN_FAILURE',
        'Unknown error saving to keychain'
      );
    }
    return { username: account, password: password };
  }),
  remove: (service, account) => runCredMan(
    'DelCred',
    {
      Target: createTargetName(service, account)
    }
  ).then(res => {
    if (res.indexOf('Successfully') !== 0) {
      throw ErrorManager.create(
        'KEYCHAIN_FAILURE',
        'Unknown error removing from keychain'
      );
    }
  })
};
