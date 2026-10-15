using FurLab.Core.Models;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Defines a service for database operations.
/// </summary>
public interface IDatabaseService
{
    /// <summary>
    /// Lists all databases on the PostgreSQL server.
    /// </summary>
    /// <param name="options">The connection options.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with a list of database names.</returns>
    Task<List<string>> ListDatabasesAsync(
        DatabaseConnectionOptions options,
        CancellationToken cancellationToken = default);
}
