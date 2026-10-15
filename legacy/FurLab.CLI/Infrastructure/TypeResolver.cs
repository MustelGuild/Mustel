using Spectre.Console.Cli;

namespace FurLab.CLI.Infrastructure;

/// <summary>
/// Resolves types from the Microsoft.Extensions.DependencyInjection service provider.
/// </summary>
public sealed class TypeResolver(IServiceProvider provider) : ITypeResolver
{
    /// <inheritdoc/>
    public object? Resolve(Type? type) => type == null ? null : provider.GetService(type);
}
