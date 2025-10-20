// Performance chart component for real-time metrics visualization

import React from 'react';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  ChartOptions,
} from 'chart.js';
import { Line } from 'react-chartjs-2';
import { ChartDataPoint } from '../types/performance';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend
);

interface PerformanceChartProps {
  data: ChartDataPoint[];
  height?: number;
}

export const PerformanceChart: React.FC<PerformanceChartProps> = ({ 
  data, 
  height = 300 
}) => {
  const formatTime = (timestamp: number) => {
    return new Date(timestamp).toLocaleTimeString();
  };

  const chartData = {
    labels: data.map(point => formatTime(point.timestamp)),
    datasets: [
      {
        label: 'RPS',
        data: data.map(point => point.rps),
        borderColor: 'rgb(59, 130, 246)',
        backgroundColor: 'rgba(59, 130, 246, 0.1)',
        yAxisID: 'y',
        tension: 0.1,
      },
      {
        label: 'Latency (ms)',
        data: data.map(point => point.latency),
        borderColor: 'rgb(16, 185, 129)',
        backgroundColor: 'rgba(16, 185, 129, 0.1)',
        yAxisID: 'y1',
        tension: 0.1,
      },
      {
        label: 'Errors',
        data: data.map(point => point.errors),
        borderColor: 'rgb(239, 68, 68)',
        backgroundColor: 'rgba(239, 68, 68, 0.1)',
        yAxisID: 'y2',
        tension: 0.1,
      },
    ],
  };

  const options: ChartOptions<'line'> = {
    responsive: true,
    maintainAspectRatio: false,
    interaction: {
      mode: 'index' as const,
      intersect: false,
    },
    plugins: {
      legend: {
        position: 'top' as const,
      },
      title: {
        display: true,
        text: 'Real-time Performance Metrics',
      },
      tooltip: {
        callbacks: {
          afterLabel: (context) => {
            if (context.datasetIndex === 0) return 'requests/sec';
            if (context.datasetIndex === 1) return 'milliseconds';
            if (context.datasetIndex === 2) return 'errors';
            return '';
          },
        },
      },
    },
    scales: {
      x: {
        display: true,
        title: {
          display: true,
          text: 'Time',
        },
      },
      y: {
        type: 'linear' as const,
        display: true,
        position: 'left' as const,
        title: {
          display: true,
          text: 'RPS',
          color: 'rgb(59, 130, 246)',
        },
        grid: {
          drawOnChartArea: false,
        },
      },
      y1: {
        type: 'linear' as const,
        display: true,
        position: 'right' as const,
        title: {
          display: true,
          text: 'Latency (ms)',
          color: 'rgb(16, 185, 129)',
        },
        grid: {
          drawOnChartArea: false,
        },
      },
      y2: {
        type: 'linear' as const,
        display: false,
        position: 'right' as const,
      },
    },
    animation: {
      duration: 0, // Disable animations for real-time updates
    },
  };

  if (data.length === 0) {
    return (
      <div 
        className="performance-chart-placeholder"
        style={{ height, display: 'flex', alignItems: 'center', justifyContent: 'center' }}
      >
        <p className="text-gray-500">No performance data available. Start a test to see real-time metrics.</p>
      </div>
    );
  }

  return (
    <div className="performance-chart" style={{ height }}>
      <Line data={chartData} options={options} />
    </div>
  );
};