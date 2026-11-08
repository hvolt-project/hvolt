# builder-optional-param

`parseGlobalBuilderParams` reads the parameter's `name` identifier, so an optional parameter
(`text?: string`) still produces `const __text__ = text;` and the third arrow parameter. The
parameter list is carried as written (`text?: string`), because the helper keeps its declared
types.
