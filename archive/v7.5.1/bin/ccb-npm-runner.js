"use strict";

const childProcess = require("child_process");
const { executablePath, install } = require("./ccb-npm-install");

async function run(command) {
  await install();
  const target = executablePath(command);
  const child = childProcess.spawn(target, process.argv.slice(2), {
    stdio: "inherit",
    env: process.env,
  });
  child.on("error", (error) => {
    console.error(error.message || error);
    process.exit(1);
  });
  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code === null ? 1 : code);
  });
}

module.exports = {
  run,
};
