import json
import os
import subprocess

# Load the template config file
with open("most.json", "r") as f:
    # template = json.load(f)
    # just load the raw text
    template = f.read()

full_log = open("full_log.txt", "w")

# Iterate over files in the "nets/" directory
for filename in os.listdir("nets"):
    file_path = os.path.join("nets", filename)

    # Check if it's a file (not a directory)
    if os.path.isfile(file_path):
        if not "boot" in filename or not "ataxx" in filename:
            continue
        # Create a new config file with the NET_PATH text replaced
        config = template.replace("NET_PATH", filename)
        with open("test.json", "w") as f:
            f.write(config)

        # Run the command and capture the last 10 lines of output
        try:
            print(f"Running test for {filename}")
            result = subprocess.run(["../external/cuteataxx/cuteataxx-cli", "test.json"], capture_output=True, text=True)
            # wait for the process to finish
            result.check_returncode()
            # split the output into lines
            output = result.stdout.split("\n")
            # take the last 10 lines
            elo_line = output[-14:][0]
            print(elo_line)
            # this line is in the format "490.71 +/- 102.58"
            # it can sometimes be inf / -inf
            elo, error = elo_line.split(" +/- ")
            full_log.write(f"{filename},{elo},{error}\n")
        except subprocess.CalledProcessError as e:
            print(f"Error executing command: {e}")
            elo_line = f"Error executing command: {e.output}"

        # Write the last 10 lines to a log file
        log_filename = f"logs/{filename}.log"
        with open(log_filename, "w") as f:
            f.write(elo_line)
            print(f"Wrote {log_filename}")