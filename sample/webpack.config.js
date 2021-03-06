// eslint-disable-next-line @typescript-eslint/no-var-requires
const path = require('path');

module.exports = {
	mode: 'development',
	entry: './src/index.ts',
	module: {
		rules: [
			{
				test: /\.ts$/,
				use: 'ts-loader',
				exclude: /node_modules/
			}
		]
	},
	resolve: {
		extensions: ['.tsx', '.ts', '.js']
	},
	output: {
		filename: 'bundle.js',
		path: path.resolve(__dirname, 'dist')
	},
	devServer: {
		liveReload: false,
		port: 3001,
		https: false,
		allowedHosts: ['vulcangames.fun']
	},
	devtool: "source-map"
};
