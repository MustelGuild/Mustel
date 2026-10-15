using System.ComponentModel;
using System.Diagnostics;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Claude.Install;

/// <summary>
/// Installs Claude Code using winget.
/// </summary>
public sealed class ClaudeInstallCommand : AsyncCommand<ClaudeInstallSettings>
{
    private static readonly List<string> WingetInstallArguments =
    [
        "install", "--id", "Anthropic.ClaudeCode", "-e", "--accept-package-agreements", "--accept-source-agreements"
    ];

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, ClaudeInstallSettings settings, CancellationToken cancellation)
    {
        if (!OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("The 'claude install' command is only supported on Windows because it uses winget.");
        }

        var result = RunProcess("winget", WingetInstallArguments);
        if (result.ExitCode != 0)
        {
            throw new Exception($"Failed to install Claude Code with winget. Exit code: {result.ExitCode}.");
        }

        return Task.FromResult(0);
    }

    private static (int ExitCode, string Output, string Error) RunProcess(string fileName, List<string> arguments)
    {
        try
        {
            using var process = new Process();
            process.StartInfo = new ProcessStartInfo
            {
                FileName = fileName,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false,
                CreateNoWindow = true
            };
            foreach (var arg in arguments)
            {
                process.StartInfo.ArgumentList.Add(arg);
            }

            process.Start();
            var output = process.StandardOutput.ReadToEnd();
            var error = process.StandardError.ReadToEnd();
            process.WaitForExit();

            if (!string.IsNullOrWhiteSpace(output))
            {
                Console.WriteLine(output.TrimEnd());
            }

            if (!string.IsNullOrWhiteSpace(error))
            {
                Console.WriteLine(error.TrimEnd());
            }

            return (process.ExitCode, output, error);
        }
        catch (Win32Exception ex)
        {
            throw new InvalidOperationException($"Could not execute '{fileName}'. Please check if the command exists in PATH.", ex);
        }
    }
}
