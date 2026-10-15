using System.Diagnostics;
using System.Text;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Winget.Restore;

/// <summary>
/// Restores winget packages from a backup file.
/// </summary>
public sealed class WingetRestoreCommand : AsyncCommand<WingetRestoreSettings>
{
    private const string BackupFileName = "backup-winget.json";

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, WingetRestoreSettings settings, CancellationToken cancellation)
    {
        var inputFile = settings.Input ?? Path.Combine(Environment.CurrentDirectory, BackupFileName);

        if (!File.Exists(inputFile))
        {
            Console.WriteLine($"Error: Backup file not found: {inputFile}");
            return Task.FromResult(2);
        }

        Console.WriteLine($"Starting winget import from: {inputFile}");

        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "winget",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true,
                StandardOutputEncoding = Encoding.UTF8
            }
        };
        process.StartInfo.ArgumentList.Add("import");
        process.StartInfo.ArgumentList.Add("-i");
        process.StartInfo.ArgumentList.Add(inputFile);

        process.Start();

        var output = process.StandardOutput.ReadToEnd();
        var error = process.StandardError.ReadToEnd();

        process.WaitForExit();

        if (!string.IsNullOrEmpty(output))
        {
            Console.WriteLine(output);
        }

        if (!string.IsNullOrEmpty(error))
        {
            Console.WriteLine($"Error: {error}");
        }

        Console.WriteLine($"\nRestore completed with exit code: {process.ExitCode}");
        return Task.FromResult(process.ExitCode == 0 ? 0 : 1);
    }
}
