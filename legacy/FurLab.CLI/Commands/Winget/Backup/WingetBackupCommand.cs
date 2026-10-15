using System.Diagnostics;
using System.Text;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Winget.Backup;

/// <summary>
/// Backs up installed winget packages.
/// </summary>
public sealed class WingetBackupCommand : AsyncCommand<WingetBackupSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, WingetBackupSettings settings, CancellationToken cancellation)
    {
        var outputDir = settings.Output ?? Environment.CurrentDirectory;
        var backupPath = Path.Combine(outputDir, "backup-winget.json");

        Console.WriteLine($"Starting winget export to: {backupPath}");

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
        process.StartInfo.ArgumentList.Add("export");
        process.StartInfo.ArgumentList.Add("-o");
        process.StartInfo.ArgumentList.Add(backupPath);
        process.StartInfo.ArgumentList.Add("--source");
        process.StartInfo.ArgumentList.Add("winget");

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

        if (process.ExitCode == 0 && File.Exists(backupPath))
        {
            Console.WriteLine($"\nBackup completed successfully!");
            Console.WriteLine($"File: {backupPath}");
        }
        else
        {
            Console.WriteLine($"\nBackup failed with exit code: {process.ExitCode}");
        }

        return Task.FromResult(process.ExitCode == 0 ? 0 : 1);
    }
}
