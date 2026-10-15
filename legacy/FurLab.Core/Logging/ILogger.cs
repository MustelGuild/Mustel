
namespace FurLab.Core.Logging;

/// <summary>
/// Defines a logging interface for the FurLab application.
/// </summary>
public interface ILogger
{
    /// <summary>
    /// Logs an informational message.
    /// </summary>
    /// <param name="message">The message to log.</param>
    /// <param name="args">Optional format arguments.</param>
    void LogInformation(string message, params object[] args);

    /// <summary>
    /// Logs a warning message.
    /// </summary>
    /// <param name="message">The message to log.</param>
    /// <param name="args">Optional format arguments.</param>
    void LogWarning(string message, params object[] args);

    /// <summary>
    /// Logs an error message.
    /// </summary>
    /// <param name="message">The message to log.</param>
    /// <param name="exception">Optional exception associated with the error.</param>
    /// <param name="args">Optional format arguments.</param>
    void LogError(string message, Exception? exception = null, params object[] args);

    /// <summary>
    /// Logs a debug message.
    /// </summary>
    /// <param name="message">The message to log.</param>
    /// <param name="args">Optional format arguments.</param>
    void LogDebug(string message, params object[] args);
}
