// Terminal Commands — keyword → { description, handler }
// Add new commands here. Handlers receive (args, output) .

export const commands = {
  echo: {
    description: "Echo back what you type",
    handler(args, output) {
      output(`  ${args}`);
    },
  },

  help: {
    description: "Show available commands",
    handler(_args, output) {
      output("  Available commands:");
      for (const [name, cmd] of Object.entries(commands)) {
        output(`    ${name}  —  ${cmd.description}`);
      }
    },
  },

  cat: {
    description: "Concatenate files",
    handler(args, output) {
      if (args === "message.txt") {
        output("  Hi, I'm Mon! I can do fullstack development, but I specialize in backend development.");
      } else if (args.startsWith("specialties/")) {
        output(`  cat: ${args}: Permission denied`);
      } else {
        output(`  cat: ${args || "(no file)"}: No such file`);
      }
    },
  },

  cd: {
    description: "Change directory",
    handler(_args, output) {
      output("  You cannot change directory");
    },
  },

  ssh: {
    description: "SSH into a remote host",
    handler(args, output) {
      const targets = {
        "monbuticloud@projects.local": "projects.html",
        "monbuticloud@contact.local": "contact.html",
      };
      const page = targets[args.trim()];
      if (page) {
        output(`  Connecting to ${args.trim()}...`);
        setTimeout(() => {
          window.location.href = page;
        }, 600);
      } else {
        const host = args.split("@").pop() || args;
        output(`  ssh: connect to host ${host} port 22: Connection refused`);
      }
    },
  },

  clear: {
    description: "Clear the terminal",
    handler(_args, _output) {
      const history = document.getElementById("term-history");
      if (history) history.innerHTML = "";
    },
  },
  // Add more commands here as needed
};
