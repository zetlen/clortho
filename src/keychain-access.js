'use strict';
const { isOSX, isWindows, hasPowershell } = require('./os-features');
const OSXKeychainManager = require('./osx-keychain-manager');
const PowershellKeychainManager = require('./powershell-keychain-manager');
const InMemoryKeychainManager = require('./in-memory-keychain-manager');

const getOSKeychainManager = () => {
  if (isOSX) return OSXKeychainManager;
  if (isWindows && hasPowershell) return PowershellKeychainManager;
  return InMemoryKeychainManager;
};

module.exports = getOSKeychainManager();
