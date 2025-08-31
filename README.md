# starframe-vault - Star Frame Counter Program

A simple counter program built with the Star Frame framework for Solana.

## Features

- ✅ Initialize counter with optional starting value
- ✅ Increment counter with overflow protection
- ✅ Decrement counter with underflow protection
- ✅ Authority-based access control
- ✅ Type-safe account validation
- ✅ Compile-time instruction verification

## Getting Started

### Prerequisites

- Rust 1.84.1+
- Solana CLI tools
- Star Frame CLI

### Building

```bash
starpin build
```

### Testing

```bash
starpin test
```

### Deploying

To devnet:
```bash
starpin deploy
```

To mainnet:
```bash
starpin deploy --mainnet
```

### Generate IDL

```bash
starpin idl
```

## Program Structure

- `CounterAccount`: Program account storing authority and count
- `Initialize`: Initialize a new counter
- `Increment`: Increment the counter value
- `Decrement`: Decrement the counter value

## Security Features

- Authority validation for all operations
- Overflow/underflow protection
- Type-safe account handling
- Compile-time validation

## Program ID

```
GxpAtbXpkbDu5b86TidcmuF5RF9UJm821rqJ5W3S4T12
```
