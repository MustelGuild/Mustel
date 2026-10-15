using System.Diagnostics;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.List;

/// <summary>
/// Lists all available Windows features, optionally filtering to enabled only.
/// </summary>
public sealed class WindowsFeaturesListCommand : AsyncCommand<WindowsFeaturesListSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, WindowsFeaturesListSettings settings, CancellationToken cancellation)
    {
        Console.WriteLine("Retrieving Windows features...");

        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "dism.exe",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true
            }
        };
        process.StartInfo.ArgumentList.Add("/online");
        process.StartInfo.ArgumentList.Add("/get-features");
        process.StartInfo.ArgumentList.Add("/format:table");

        process.Start();

        var output = process.StandardOutput.ReadToEnd();
        var error = process.StandardError.ReadToEnd();

        process.WaitForExit();

        if (process.ExitCode != 0)
        {
            if (!string.IsNullOrEmpty(error))
            {
                Console.WriteLine(error);
            }

            Console.WriteLine("Error retrieving features. Make sure you're running as Administrator.");
            return Task.FromResult(1);
        }

        if (!string.IsNullOrEmpty(error))
        {
            Console.WriteLine(error);
        }

        Console.WriteLine(output);

        return Task.FromResult(0);
    }
}
