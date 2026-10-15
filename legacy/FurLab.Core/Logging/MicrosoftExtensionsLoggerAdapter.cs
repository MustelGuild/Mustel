using Microsoft.Extensions.Logging;

namespace FurLab.Core.Logging;

/// <summary>
/// Adapter for Microsoft.Extensions.Logging.ILogger to FurLab.Core.Logging.ILogger.
/// </summary>
public sealed class MicrosoftExtensionsLoggerAdapter(Microsoft.Extensions.Logging.ILogger logger) : ILogger
{
    private readonly Microsoft.Extensions.Logging.ILogger _logger = logger ?? throw new ArgumentNullException(nameof(logger));

    /// <inheritdoc/>
    public void LogInformation(string message, params object[] args)
    {
        _logger.LogInformation(message, args);
    }

    /// <inheritdoc/>
    public void LogWarning(string message, params object[] args)
    {
        _logger.LogWarning(message, args);
    }

    /// <inheritdoc/>
    public void LogError(string message, Exception? exception = null, params object[] args)
    {
        if (exception != null)
        {
            _logger.LogError(exception, message, args);
        }
        else
        {
            _logger.LogError(message, args);
        }
    }

    /// <inheritdoc/>
    public void LogDebug(string message, params object[] args)
    {
        _logger.LogDebug(message, args);
    }
}
