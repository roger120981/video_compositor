import { LoggerLevel } from '@live-compositor/core';
import { pino, type Logger } from 'pino';

const PRETTY_TRANSPORT = {
  target: 'pino-pretty',
  options: {
    colorize: true,
  },
};

function getLoggerLevel(): LoggerLevel {
  const logLevel =
    process.env.LIVE_COMPOSITOR_LOGGER_LEVEL ?? (process.env.DEBUG ? 'debug' : undefined);
  if (Object.values(LoggerLevel).includes(logLevel as LoggerLevel)) {
    return logLevel as LoggerLevel;
  } else {
    return LoggerLevel.WARN;
  }
}

type LoggerFormat = 'json' | 'pretty' | 'compact';

function getLoggerFormat(): LoggerFormat {
  const env = process.env.LIVE_COMPOSITOR_LOGGER_FORMAT;
  return ['json', 'compact', 'pretty'].includes(process.env.LIVE_COMPOSITOR_LOGGER_FORMAT ?? '')
    ? (env as LoggerFormat)
    : 'json';
}

export function createLogger(): Logger {
  return pino({
    level: getLoggerLevel(),
    transport: ['pretty', 'compact'].includes(getLoggerFormat()) ? PRETTY_TRANSPORT : undefined,
  });
}

export function compositorInstanceLoggerOptions(): {
  format: string;
  level: string;
} {
  const loggerLevel = getLoggerLevel();
  const format = getLoggerFormat();

  if ([LoggerLevel.WARN, LoggerLevel.ERROR].includes(loggerLevel)) {
    return {
      level: loggerLevel,
      format,
    };
  } else if (loggerLevel === LoggerLevel.INFO) {
    return {
      level: 'warn',
      format,
    };
  } else if (loggerLevel === LoggerLevel.DEBUG) {
    return {
      // hide scene update request with "compositor_pipeline::pipeline=warn"
      level: 'info,wgpu_hal=warn,wgpu_core=warn,compositor_pipeline::pipeline=warn',
      format,
    };
  } else if (loggerLevel === LoggerLevel.TRACE) {
    return {
      level: 'debug,wgpu_hal=warn,wgpu_core=warn,naga=warn,live_compositor::log_request_body=trace',
      format,
    };
  } else {
    return {
      level: 'error',
      format: 'json',
    };
  }
}
