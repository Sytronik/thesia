#!/usr/bin/env python3
# -*- coding:utf-8 -*-

import base64
import io
import json
import multiprocessing as mp
import os
from argparse import ArgumentParser

import dash
import dash_core_components as dcc
import dash_html_components as html
import numpy as np
import plotly.graph_objects as go
import soundfile as sf
from dash.dependencies import Input, Output, State
# from flask_caching import Cache
from plotly.subplots import make_subplots
from threadpoolctl import threadpool_limits

from spec import Spectrogram

DEFAULT_VALUES = dict(
    win_ms=40,
    overlap=4,
    n_mel=128,
)
WINDOW_LENGTHS = [5, 10, 20, 30, 40, 50, 80, 160]
OVERLAPS = [2, 4, 8, 16]

specs = []
fig = None

external_stylesheets = ['https://codepen.io/chriddyp/pen/bWLwgP.css']
app = dash.Dash(__name__, external_stylesheets=external_stylesheets)
# cache = Cache(app.server, config={
#     # try 'filesystem' if you don't want to setup redis
#     'CACHE_TYPE': 'filesystem',
#     'CACHE_DIR': './cache',
#     # 'CACHE_REDIS_URL': os.environ.get('REDIS_URL', '')
# })
app.config.suppress_callback_exceptions = True
app.layout = html.Div([
    dcc.Upload(
        html.Div(['Drag and Drop or ', html.A('Select Files')]),
        id='upload-audio',
        style={
            'width': '100%',
            'height': '60px',
            'lineHeight': '60px',
            'borderWidth': '1px',
            'borderStyle': 'dashed',
            'borderRadius': '5px',
            'textAlign': 'center',
            'margin': '10px'
        },
        multiple=True,
    ),
    html.Div([
        html.Div([
            html.Label('Frequency Scale', style={'display': 'block'}),
            dcc.RadioItems(
                id='freq_scale',
                options=[
                    {'label': 'Mel', 'value': 'mel'},
                    {'label': 'Linear', 'value': 'linear'},
                ],
                value='mel',
                labelStyle={'display': 'inline-block', 'padding-right': 10},
                style={
                    'height': 36,
                    'display': 'flex',
                    'align-items': 'center',
                },
            ),
        ], style={'flex': '25%'}),
        html.Div([
            html.Label('Window Length', style={'display': 'block'}),
            dcc.Dropdown(
                id='win_ms',
                options=[{'label': f'{i} ms', 'value': i} for i in WINDOW_LENGTHS],
                value=DEFAULT_VALUES['win_ms'],
                clearable=False,
                style={'width': '90%'},
            ),
        ], style={'flex': '25%'}),
        html.Div([
            html.Label('Overlap'),
            dcc.Dropdown(
                id='overlap',
                options=[{'label': f'{i}x', 'value': i} for i in OVERLAPS],
                value=DEFAULT_VALUES['overlap'],
                clearable=False,
                style={'width': '90%'},
            ),
        ], style={'flex': '25%'}),
        html.Div([
            html.Label('No. of Mel Filterbanks'),
            dcc.Input(
                id='n_mel',
                type='number',
                min=10,
                step=1,
                value=DEFAULT_VALUES['n_mel'],
                debounce=True,
                style={'width': '90%'},
            ),
        ], style={'flex': '25%'}),
    ], style={'display': 'flex', 'padding-bottom': 20}),
    dcc.Graph(figure=go.Figure(), id='graphs'),
])


def parse_contents(contents, fname, win_ms, overlap, n_mel, freq_scale, n_threads=0):
    _, content_string = contents.split(',')

    decoded = base64.b64decode(content_string)
    try:
        wav, sr = sf.read(io.BytesIO(decoded))
    except Exception as e:
        print(e)
        return html.Div([
            'There was an error processing this file.'
        ])

    with threadpool_limits(limits=n_threads if n_threads > 0 else None, user_api='blas'):
        spec = Spectrogram(wav, sr, win_ms, overlap, n_mel)
        heatmap = go.Heatmap(
            x=spec.t_axis,
            y=None if freq_scale == 'mel' else spec.f_linear_axis,
            z=spec.mel if freq_scale == 'mel' else spec.linear,
            customdata=spec.f_mel_axis_str if freq_scale == 'mel' else spec.f_linear_axis_str,
            hovertemplate='%{x:.3f} sec, %{customdata} Hz<br>%{z:.3f} dB',
            colorscale='Inferno',
            showscale=False,
            name=fname,
        )
        scatter = go.Scattergl(
            x=np.arange(len(spec.wav)) / sr,
            y=spec.wav,
            line_color='rgb(62,130,250)',
            hovertemplate='%{x:.3f} sec, %{y:e}',
            showlegend=False,
            name=fname,
        )
    return spec, heatmap, scatter


@app.callback(
    Output('graphs', 'figure'),
    Input('upload-audio', 'contents'),
    Input('freq_scale', 'value'),
    Input('win_ms', 'value'),
    Input('overlap', 'value'),
    Input('n_mel', 'value'),
    State('upload-audio', 'filename'),
)
# @cache.memoize(timeout=20)  # in seconds
def update_graphs(contents_list, freq_scale, win_ms, overlap, n_mel, filenames):
    global specs, fig
    if not contents_list:
        # with open('sample.wav', 'rb') as f:
        #     c = f.read()
        # contents_list = [',' + base64.b64encode(c).decode('utf-8')]
        # filenames = ['sample.wav']
        # triggered_id = 'upload-audio'
        return go.Figure()
    else:
        triggered = dash.callback_context.triggered
        if not triggered:
            return go.Figure()
        triggered_id = triggered[0]['prop_id'].split('.')[0]
    if triggered_id == 'upload-audio':
        n_threads = max(2, mp.cpu_count() // len(contents_list))
        result = pool.starmap_async(
            parse_contents,
            [(c, f, win_ms, overlap, n_mel, freq_scale, n_threads) for c, f in zip(contents_list, filenames)],
        )
        fig = make_subplots(
            rows=len(contents_list), cols=1,
            shared_xaxes=True,
            vertical_spacing=0.12 / len(contents_list),
            specs=[[{'secondary_y': True}] for _ in range(len(contents_list))],
            subplot_titles=[f'{fname}' for fname in filenames],
        )
        fig.update_layout(
            height=200*len(contents_list) + 24*len(contents_list),
            # template='plotly_dark',
            # coloraxis={'colorscale':'Inferno', 'colorbar_title': dict(text='dB')},
            margin=dict(l=0, r=0, t=30, b=0),
        )
        tup_list = result.get()
        max_amp = 0
        specs, plots = [], []
        for annotation, (spec, *plot) in zip(fig['layout']['annotations'], tup_list):
            annotation['text'] += f' (sr={spec.sr:d} Hz)'
            annotation['align'] = 'left'
            annotation['xanchor'] = 'left'
            annotation['x'] = 0
            if (amp := np.abs(spec.wav).max()) > max_amp:
                max_amp = amp
            specs.append(spec)
            plots.append(plot)
            # print(annotation['y'])
        max_amp = min(round(max_amp, ndigits=1), 0.5)
        for i, (heatmap, scatter) in enumerate(plots):
            fig.add_trace(heatmap, row=i+1, col=1)
            fig.update_yaxes(showticklabels=False,
                             row=i+1, col=1,
                             secondary_y=False,
                             )
            fig.add_trace(scatter, secondary_y=True, row=i+1, col=1)
            fig.update_yaxes(range=[-max_amp*2, max_amp*2],
                             fixedrange=True,
                             tick0=-max_amp*2, dtick=max_amp,
                             row=i+1, col=1,
                             secondary_y=True,
                             )

    for spec, data in zip(specs, fig.data[::2]):
        spec.win_ms = win_ms
        spec.overlap = overlap
        if n_mel:
            spec.n_mel = n_mel

        data.x = spec.t_axis
        if freq_scale == 'mel':
            data.z = spec.mel
            if data.y is not None:
                data.customdata = spec.f_mel_axis_str
            data.y = None
        else:
            data.z = spec.linear
            if data.y is None:
                data.customdata = spec.f_linear_axis_str
            data.y = spec.f_linear_axis

    return fig


@app.callback(
    Output('win_ms', 'options'),
    Input('graphs', 'figure'),
    State('win_ms', 'options'),
    State('win_ms', 'value'),
)
def update_n_fft(figure, options, value):
    if not specs:
        return options
    for option in options:
        if option['value'] == value:
            option['label'] = f'{value} ms (n_fft: {specs[0].n_fft})'
        else:
            option['label'] = str(option['value']) + ' ms'
    return options


@app.callback(
    Output('n_mel', 'disabled'),
    Input('freq_scale', 'value'),
)
def determine_disabling_n_mel(freq_scale):
    if freq_scale == 'mel':
        return False
    return True


if __name__ == "__main__":
    pool = mp.Pool(mp.cpu_count()//2)
    parser = ArgumentParser()
    parser.add_argument('-p', '--port', type=int, default=8080)
    args = parser.parse_args()
    app.run_server(debug=False, port=args.port, host='0.0.0.0')
