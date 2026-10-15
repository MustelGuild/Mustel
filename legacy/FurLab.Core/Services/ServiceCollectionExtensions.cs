using System.Runtime.Versioning;

using FurLab.Core.Interfaces;
using FurLab.Core.Logging;

using Microsoft.AspNetCore.DataProtection;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;

namespace FurLab.Core.Services;

/// <summary>
/// Extension methods for configuring FurLab services.
/// </summary>
public static class ServiceCollectionExtensions
{
    /// <summary>
    /// Adds FurLab core services to the dependency injection container.
    /// </summary>
    /// <param name="services">The service collection to add services to.</param>
    /// <returns>The service collection for chaining.</returns>
    public static IServiceCollection AddFurLabServices(this IServiceCollection services)
    {
        _ = services.AddLogging(builder =>
        {
            _ = builder.AddConsole();
            _ = builder.SetMinimumLevel(LogLevel.Warning);
        });

        _ = services.AddSingleton<IProcessExecutor, ProcessExecutor>();
        _ = services.AddSingleton<IDatabaseService, DatabaseService>();
        _ = services.AddSingleton<IPgPassService, PgPassService>();

        services.AddSingleton<Logging.ILogger>(sp =>
        {
            var msLogger = sp.GetRequiredService<ILogger<UserConfigService>>();
            return new MicrosoftExtensionsLoggerAdapter(msLogger);
        });

        _ = services.AddSingleton<IUserConfigService, UserConfigService>();

        var keysDir = new DirectoryInfo(Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "FurLab",
            "keys"));

        var dpBuilder = services.AddDataProtection()
            .PersistKeysToFileSystem(keysDir)
            .SetApplicationName("FurLab");

        if (OperatingSystem.IsWindows())
            ProtectKeysWithDpapiOnWindows(dpBuilder);

        _ = services.AddSingleton<ICredentialService, CredentialService>();

        _ = services.AddSingleton<IPostgresBinaryLocator, PostgresBinaryLocator>();
        _ = services.AddSingleton<IPostgresPasswordHandler, PostgresPasswordHandler>();
        _ = services.AddSingleton<IDockerService, Docker.DockerService>();
        _ = services.AddSingleton<IEncodingConversionService, EncodingConversionService>();

        return services;
    }

    [SupportedOSPlatform("windows")]
    private static void ProtectKeysWithDpapiOnWindows(IDataProtectionBuilder builder) =>
        builder.ProtectKeysWithDpapi(protectToLocalMachine: false);
}
