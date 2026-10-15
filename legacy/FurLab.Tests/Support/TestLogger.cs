using System;
using FurLab.Core.Logging;

namespace FurLab.Tests.Support;

/// <summary>
/// A dummy logger implementation for testing purposes.
/// </summary>
public sealed class TestLogger : ILogger
{
    public void LogInformation(string message, params object[] args) { }
    public void LogWarning(string message, params object[] args) { }
    public void LogError(string message, Exception? exception = null, params object[] args) { }
    public void LogDebug(string message, params object[] args) { }
}
