namespace FurLab.Core.Constants;

/// <summary>
/// Contains constant values used throughout the FurLab application.
/// </summary>
public static class FurLabConstants
{
    /// <summary>
    /// The default PostgreSQL host.
    /// </summary>
    public const string DefaultHost = "localhost";

    /// <summary>
    /// The default PostgreSQL port.
    /// </summary>
    public const string DefaultPort = "5432";

    /// <summary>
    /// The pg_dump executable name.
    /// </summary>
    public const string PgDumpExecutable = "pg_dump";

    /// <summary>
    /// The pg_restore executable name.
    /// </summary>
    public const string PgRestoreExecutable = "pg_restore";

    /// <summary>
    /// The psql executable name.
    /// </summary>
    public const string PsqlExecutable = "psql";

    /// <summary>
    /// The backup/dump file extension.
    /// </summary>
    public const string DumpExtension = ".dump";
}
