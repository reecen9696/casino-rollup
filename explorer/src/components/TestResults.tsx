// Test results display component for performance testing

import React from 'react';
import { TestResult } from '../types/performance';

interface TestResultsProps {
  testResult: TestResult;
  onExport?: (result: TestResult) => void;
}

export const TestResults: React.FC<TestResultsProps> = ({ testResult, onExport }) => {
  const formatDuration = (seconds: number) => {
    if (seconds < 1) return `${Math.round(seconds * 1000)}ms`;
    return `${seconds.toFixed(2)}s`;
  };

  const formatLatency = (ms: number) => {
    return `${ms.toFixed(2)}ms`;
  };

  const formatRps = (rps: number) => {
    return rps.toFixed(2);
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'completed': return 'text-green-600 bg-green-50';
      case 'running': return 'text-blue-600 bg-blue-50';
      case 'failed': return 'text-red-600 bg-red-50';
      case 'cancelled': return 'text-yellow-600 bg-yellow-50';
      default: return 'text-gray-600 bg-gray-50';
    }
  };

  const getTestTypeLabel = (config: TestResult['config']) => {
    switch (config.testType) {
      case 'burst':
        return `Burst Test (${config.totalRequests} requests)`;
      case 'sustained':
        return `Sustained Test (${config.rps} RPS × ${config.duration}s)`;
      case 'custom':
        return 'Custom Test';
      default:
        return 'Unknown Test';
    }
  };

  const handleExport = () => {
    if (onExport) {
      onExport(testResult);
    } else {
      // Default export as JSON
      const dataStr = JSON.stringify(testResult, null, 2);
      const dataBlob = new Blob([dataStr], { type: 'application/json' });
      const url = URL.createObjectURL(dataBlob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `performance-test-${testResult.id}.json`;
      link.click();
      URL.revokeObjectURL(url);
    }
  };

  return (
    <div className="test-results border border-gray-200 rounded-lg p-6 bg-white shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h3 className="text-lg font-semibold text-gray-900">
            {getTestTypeLabel(testResult.config)}
          </h3>
          <p className="text-sm text-gray-500">
            Test ID: {testResult.id}
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className={`px-3 py-1 rounded-full text-sm font-medium ${getStatusColor(testResult.status)}`}>
            {testResult.status.charAt(0).toUpperCase() + testResult.status.slice(1)}
          </span>
          {testResult.status === 'completed' && (
            <button
              onClick={handleExport}
              className="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
            >
              Export
            </button>
          )}
        </div>
      </div>

      {/* Test Configuration */}
      <div className="mb-6">
        <h4 className="text-sm font-medium text-gray-700 mb-2">Configuration</h4>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-sm">
          <div>
            <span className="text-gray-500">Type:</span>
            <span className="ml-2 font-medium">{testResult.config.testType}</span>
          </div>
          {testResult.config.totalRequests && (
            <div>
              <span className="text-gray-500">Requests:</span>
              <span className="ml-2 font-medium">{testResult.config.totalRequests}</span>
            </div>
          )}
          {testResult.config.rps && (
            <div>
              <span className="text-gray-500">Target RPS:</span>
              <span className="ml-2 font-medium">{testResult.config.rps}</span>
            </div>
          )}
          {testResult.config.duration && (
            <div>
              <span className="text-gray-500">Duration:</span>
              <span className="ml-2 font-medium">{testResult.config.duration}s</span>
            </div>
          )}
        </div>
      </div>

      {/* Metrics */}
      {testResult.metrics && Object.keys(testResult.metrics).length > 0 && (
        <div className="mb-6">
          <h4 className="text-sm font-medium text-gray-700 mb-3">Results</h4>
          
          {/* Key Metrics */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4">
            <div className="bg-blue-50 p-3 rounded">
              <div className="text-2xl font-bold text-blue-600">
                {formatRps(testResult.metrics.rps)}
              </div>
              <div className="text-sm text-blue-600">RPS</div>
            </div>
            <div className="bg-green-50 p-3 rounded">
              <div className="text-2xl font-bold text-green-600">
                {formatLatency(testResult.metrics.averageLatency)}
              </div>
              <div className="text-sm text-green-600">Avg Latency</div>
            </div>
            <div className="bg-purple-50 p-3 rounded">
              <div className="text-2xl font-bold text-purple-600">
                {testResult.metrics.successRate.toFixed(1)}%
              </div>
              <div className="text-sm text-purple-600">Success Rate</div>
            </div>
            <div className="bg-gray-50 p-3 rounded">
              <div className="text-2xl font-bold text-gray-600">
                {formatDuration(testResult.metrics.duration)}
              </div>
              <div className="text-sm text-gray-600">Duration</div>
            </div>
          </div>

          {/* Detailed Metrics */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
            <div>
              <h5 className="text-sm font-medium text-gray-700 mb-2">Request Metrics</h5>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-500">Total Requests:</span>
                  <span className="font-medium">{testResult.metrics.totalRequests}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">Successful:</span>
                  <span className="font-medium text-green-600">{testResult.metrics.successfulRequests}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">Failed:</span>
                  <span className="font-medium text-red-600">{testResult.metrics.failedRequests}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">Error Rate:</span>
                  <span className="font-medium">{testResult.metrics.errorRate.toFixed(2)}%</span>
                </div>
              </div>
            </div>

            <div>
              <h5 className="text-sm font-medium text-gray-700 mb-2">Latency Percentiles</h5>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-500">Min:</span>
                  <span className="font-medium">{formatLatency(testResult.metrics.minLatency)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">P50:</span>
                  <span className="font-medium">{formatLatency(testResult.metrics.p50Latency)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">P95:</span>
                  <span className="font-medium">{formatLatency(testResult.metrics.p95Latency)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">P99:</span>
                  <span className="font-medium">{formatLatency(testResult.metrics.p99Latency)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">Max:</span>
                  <span className="font-medium">{formatLatency(testResult.metrics.maxLatency)}</span>
                </div>
              </div>
            </div>

            <div>
              <h5 className="text-sm font-medium text-gray-700 mb-2">Bet Outcomes</h5>
              <div className="space-y-2 text-sm">
                <div className="flex justify-between">
                  <span className="text-gray-500">🪙 Heads Results:</span>
                  <span className="font-medium">{testResult.metrics.headsResults}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">🪙 Tails Results:</span>
                  <span className="font-medium">{testResult.metrics.tailsResults}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">✅ Correct Guesses:</span>
                  <span className="font-medium text-green-600">{testResult.metrics.correctGuesses}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">💰 Total Payout:</span>
                  <span className="font-medium text-purple-600">{(testResult.metrics.totalPayout / 1000000).toFixed(3)} SOL</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-gray-500">🎯 Win Rate:</span>
                  <span className="font-medium">{testResult.metrics.totalRequests > 0 ? ((testResult.metrics.correctGuesses / testResult.metrics.successfulRequests) * 100).toFixed(1) : '0'}%</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Errors */}
      {testResult.errors.length > 0 && (
        <div className="mb-4">
          <h4 className="text-sm font-medium text-red-700 mb-2">Errors</h4>
          <div className="bg-red-50 border border-red-200 rounded p-3">
            {testResult.errors.map((error, index) => (
              <div key={index} className="text-sm text-red-700">
                {error}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Timestamps */}
      <div className="text-xs text-gray-500 border-t pt-3">
        <div className="flex justify-between">
          <span>Started: {testResult.createdAt.toLocaleString()}</span>
          {testResult.completedAt && (
            <span>Completed: {testResult.completedAt.toLocaleString()}</span>
          )}
        </div>
      </div>
    </div>
  );
};