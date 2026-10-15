using System.Diagnostics;
using System.Text.Json;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.Import;

/// <summary>
/// Imports and enables Windows features from a previously exported JSON file.
/// </summary>
public sealed class WindowsFeaturesImportCommand : AsyncCommand<WindowsFeaturesImportSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, WindowsFeaturesImportSettings settings, CancellationToken cancellation)
    {
        if (!File.Exists(settings.Path))
        {
            Console.WriteLine($"Error: File not found: {settings.Path}");
            return Task.FromResult(2);
        }

        var json = File.ReadAllText(settings.Path);
        var exportData = JsonSerializer.Deserialize<WindowsFeaturesExport>(json);

        if (exportData?.Features == null || exportData.Features.Count == 0)
        {
            Console.WriteLine("No features found in the import file.");
            return Task.FromResult(1);
        }

        Console.WriteLine($"Importing {exportData.Features.Count} features...");
        var successCount = 0;
        var failCount = 0;

        foreach (var feature in exportData.Features)
        {
            Console.Write($"Enabling {feature}... ");
            var result = RunDismCommand(["/online", "/enable-feature", $"/featurename:{feature}", "/All"]);
            if (result == 0)
            {
                Console.WriteLine("OK");
                successCount++;
            }
            else
            {
                Console.WriteLine($"FAILED (exit code: {result})");
                failCount++;
            }
        }

        Console.WriteLine($"\nImport complete. Success: {successCount}, Failed: {failCount}");
        if (failCount > 0)
        {
            Console.WriteLine("Some features may require a restart or elevated permissions.");
        }

        return Task.FromResult(failCount > 0 ? 1 : 0);
    }

    private static int RunDismCommand(List<string> arguments)
    {
        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "dism.exe",
                UseShellExecute = true,
                RedirectStandardOutput = false,
                RedirectStandardError = false,
                CreateNoWindow = false
            }
        };
        foreach (var arg in arguments)
        {
            process.StartInfo.ArgumentList.Add(arg);
        }

        process.Start();
        process.WaitForExit();
        return process.ExitCode;
    }
}
