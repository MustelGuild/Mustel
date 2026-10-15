using FurLab.Core.Models;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Defines a service for executing external processes.
/// </summary>
public interface IProcessExecutor
{
    /// <summary>
    /// Executes a process with the specified options.
    /// </summary>
    /// <param name="options">The execution options.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with the result.</returns>
    Task<ProcessExecutionResult> ExecuteAsync(
        ProcessExecutionOptions options,
        CancellationToken cancellationToken = default);
}
