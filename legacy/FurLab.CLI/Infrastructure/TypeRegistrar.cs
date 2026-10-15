using Microsoft.Extensions.DependencyInjection;

using Spectre.Console.Cli;

namespace FurLab.CLI.Infrastructure;

/// <summary>
/// Bridges Microsoft.Extensions.DependencyInjection to Spectre.Console.Cli's type resolution.
/// </summary>
public sealed class TypeRegistrar(IServiceCollection services) : ITypeRegistrar
{
    /// <inheritdoc/>
    public ITypeResolver Build() => new TypeResolver(services.BuildServiceProvider());

    /// <inheritdoc/>
    public void Register(Type service, Type implementation) =>
        services.AddSingleton(service, implementation);

    /// <inheritdoc/>
    public void RegisterInstance(Type service, object implementation) =>
        services.AddSingleton(service, implementation);

    /// <inheritdoc/>
    public void RegisterLazy(Type service, Func<object> factory) =>
        services.AddSingleton(service, _ => factory());
}
