'use strict';

var _require = require('./os-features'),
    isOSX = _require.isOSX,
    isWindows = _require.isWindows,
    hasPowershell = _require.hasPowershell;

var OSXKeychainManager = require('./osx-keychain-manager');
var PowershellKeychainManager = require('./powershell-keychain-manager');
var InMemoryKeychainManager = require('./in-memory-keychain-manager');

var getOSKeychainManager = function getOSKeychainManager() {
  if (isOSX) return OSXKeychainManager;
  if (isWindows && hasPowershell) return PowershellKeychainManager;
  return InMemoryKeychainManager;
};

module.exports = getOSKeychainManager();