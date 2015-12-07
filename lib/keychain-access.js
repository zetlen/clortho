'use strict';

var _require = require('./os-features');

var isOSX = _require.isOSX;
var isWindows = _require.isWindows;
var hasPowershell = _require.hasPowershell;

var OSXKeychainManager = require('./osx-keychain-manager');
var PowershellKeychainManager = require('./powershell-keychain-manager');
var InMemoryKeychainManager = require('./in-memory-keychain-manager');

var getOSKeychainManager = function getOSKeychainManager() {
  if (isOSX) return OSXKeychainManager;
  if (isWindows && hasPowershell) return PowershellKeychainManager;
  return InMemoryKeychainManager;
};

module.exports = getOSKeychainManager();