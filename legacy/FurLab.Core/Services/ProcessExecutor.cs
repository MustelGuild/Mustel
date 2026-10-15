using System.Diagnostics;
using System.Text;

using FurLab.Core.Interfaces;
using FurLab.Core.Logging;
using FurLab.Core.Models;

namespace FurLab.Core.Services;

/// <summary>
/// Provides methods for executing external processes.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="ProcessExecutor"/> class.
/// </remarks>
/// <param name="logger">The logger instance.</param>
public class ProcessExecutor(ILogger logger) : IProcessExecutor
{
    private readonly ILogger _logger = logger ?? throw new ArgumentNullException(nameof(logger));

    /// <summary>
    /// Executes a process with the specified options.
    /// </summary>
    /// <param name="options">The execution options.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with the result.</returns>
    public async Task<ProcessExecutionResult> ExecuteAsync(
        ProcessExecutionOptions options,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(options);

        _logger.LogDebug($"Starting process: {options.FileName} {options.Arguments}");

        var startInfo = new ProcessStartInfo
        {
            FileName = options.FileName,
            RedirectStandardOutput = options.RedirectStandardOutput,
            RedirectStandardError = options.RedirectStandardError,
            UseShellExecute = false,
            CreateNoWindow = options.CreateNoWindow
        };

        if (options.Arguments is not null && options.Arguments.Count > 0)
        {
            foreach (var arg in options.Arguments)
            {
                startInfo.ArgumentList.Add(arg);
            }
        }

        if (!string.IsNullOrWhiteSpace(options.WorkingDirectory))
        {
            startInfo.WorkingDirectory = options.WorkingDirectory;
        }

        if (options.EnvironmentVariables != null)
        {
            foreach (var kvp in options.EnvironmentVariables)
            {
                startInfo.Environment[kvp.Key] = kvp.Value;
            }
        }

        using var process = new Process();
        process.StartInfo = startInfo;

        var outputBuilder = new StringBuilder();
        var errorBuilder = new StringBuilder();

        if (options.RedirectStandardOutput)
        {
            process.OutputDataReceived += (sender, e) =>
            {
                if (!string.IsNullOrEmpty(e.Data))
                {
                    outputBuilder.AppendLine(e.Data);
                }
            };
        }

        if (options.RedirectStandardError)
        {
            process.ErrorDataReceived += (sender, e) =>
            {
                if (!string.IsNullOrEmpty(e.Data))
                {
                    errorBuilder.AppendLine(e.Data);
                }
            };
        }

        try
        {
            process.Start();

            if (options.RedirectStandardOutput)
            {
                process.BeginOutputReadLine();
            }

            if (options.RedirectStandardError)
            {
                process.BeginErrorReadLine();
            }

            // Wait for process to complete with cancellation support
            var waitForExitTask = process.WaitForExitAsync(cancellationToken);
            await waitForExitTask.ConfigureAwait(false);

            var result = new ProcessExecutionResult
            {
                ExitCode = process.ExitCode,
                StandardOutput = outputBuilder.ToString(),
                StandardError = errorBuilder.ToString()
            };

            _logger.LogDebug($"Process completed with exit code: {process.ExitCode}");

            return result;
        }
        catch (OperationCanceledException)
        {
            _logger.LogWarning("Process execution was cancelled");

            try
            {
                if (!process.HasExited)
                {
                    process.Kill(entireProcessTree: true);
                }
            }
            catch
            {
                // Ignore errors when killing the process
            }

            return new ProcessExecutionResult
            {
                ExitCode = -1,
                StandardOutput = outputBuilder.ToString(),
                StandardError = "Operation was cancelled"
            };
        }
        catch (Exception ex)
        {
            _logger.LogError($"Process execution failed: {ex.Message}");

            return new ProcessExecutionResult
            {
                ExitCode = -1,
                StandardOutput = outputBuilder.ToString(),
                StandardError = ex.Message
            };
        }
    }
}
