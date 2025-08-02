#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
币安连接测试脚本
测试网络是否可以连接币安交易所并获取公开数据（不需要API密钥）
"""

import requests
import json
import time
from datetime import datetime

def test_binance_rest_api():
    """测试币安REST API连接"""
    print("=== 测试币安REST API连接 ===")
    
    # 币安现货API基础URL
    base_url = "https://api.binance.com"
    
    try:
        # 1. 测试服务器时间
        print("1. 获取服务器时间...")
        response = requests.get(f"{base_url}/api/v3/time", timeout=10)
        if response.status_code == 200:
            server_time = response.json()
            print(f"   ✅ 服务器时间: {datetime.fromtimestamp(server_time['serverTime']/1000)}")
        else:
            print(f"   ❌ 获取服务器时间失败: {response.status_code}")
            return False
            
        # 2. 测试交易对信息
        print("2. 获取交易对信息...")
        response = requests.get(f"{base_url}/api/v3/exchangeInfo", timeout=10)
        if response.status_code == 200:
            exchange_info = response.json()
            symbols_count = len(exchange_info['symbols'])
            print(f"   ✅ 获取到 {symbols_count} 个交易对")
        else:
            print(f"   ❌ 获取交易对信息失败: {response.status_code}")
            return False
            
        # 3. 测试24小时价格统计
        print("3. 获取BTCUSDT 24小时价格统计...")
        response = requests.get(f"{base_url}/api/v3/ticker/24hr?symbol=BTCUSDT", timeout=10)
        if response.status_code == 200:
            ticker = response.json()
            print(f"   ✅ BTCUSDT 当前价格: {ticker['lastPrice']} USDT")
            print(f"   ✅ 24小时涨跌幅: {ticker['priceChangePercent']}%")
        else:
            print(f"   ❌ 获取价格统计失败: {response.status_code}")
            return False
            
        # 4. 测试订单簿
        print("4. 获取BTCUSDT订单簿...")
        response = requests.get(f"{base_url}/api/v3/depth?symbol=BTCUSDT&limit=5", timeout=10)
        if response.status_code == 200:
            depth = response.json()
            print(f"   ✅ 买一价: {depth['bids'][0][0]} USDT")
            print(f"   ✅ 卖一价: {depth['asks'][0][0]} USDT")
        else:
            print(f"   ❌ 获取订单簿失败: {response.status_code}")
            return False
            
        return True
        
    except requests.exceptions.RequestException as e:
        print(f"   ❌ 网络连接错误: {e}")
        return False
    except Exception as e:
        print(f"   ❌ 其他错误: {e}")
        return False

def test_binance_websocket():
    """测试币安WebSocket连接"""
    print("\n=== 测试币安WebSocket连接 ===")
    
    try:
        import websocket
        import threading
        import time
        
        # WebSocket连接状态
        ws_connected = False
        message_count = 0
        test_complete = False
        
        def on_message(ws, message):
            nonlocal message_count, test_complete
            message_count += 1
            try:
                data = json.loads(message)
                if 'c' in data:  # 最新价格
                    print(f"   📈 BTCUSDT 实时价格: {data['c']} USDT (消息#{message_count})")
                if message_count >= 2:  # 接收2条消息后关闭
                    test_complete = True
                    ws.close()
            except:
                pass
                
        def on_error(ws, error):
            print(f"   ❌ WebSocket错误: {error}")
            
        def on_close(ws, close_status_code, close_msg):
            print(f"   🔌 WebSocket连接已关闭")
            
        def on_open(ws):
            nonlocal ws_connected
            ws_connected = True
            print(f"   ✅ WebSocket连接成功")
            
        # 币安WebSocket URL - 使用简单的ticker流
        ws_url = "wss://stream.binance.com:9443/ws/btcusdt@ticker"
        
        print(f"正在连接到: {ws_url}")
        
        # 设置WebSocket选项
        websocket.enableTrace(False)
        ws = websocket.WebSocketApp(ws_url,
                                  on_open=on_open,
                                  on_message=on_message,
                                  on_error=on_error,
                                  on_close=on_close)
        
        # 在单独线程中运行WebSocket
        def run_ws():
            ws.run_forever(ping_interval=30, ping_timeout=10)
            
        ws_thread = threading.Thread(target=run_ws)
        ws_thread.daemon = True
        ws_thread.start()
        
        # 等待最多10秒
        for i in range(100):  # 10秒，每100ms检查一次
            time.sleep(0.1)
            if test_complete or message_count >= 2:
                break
                
        if ws_connected:
            ws.close()
            
        return ws_connected and message_count > 0
        
    except ImportError:
        print("   ⚠️ 需要安装websocket-client: pip install websocket-client")
        return False
    except Exception as e:
        print(f"   ❌ WebSocket测试失败: {e}")
        return False

def main():
    """主函数"""
    print("🚀 开始测试币安连接...")
    print(f"测试时间: {datetime.now()}")
    print("-" * 50)
    
    # 测试REST API
    rest_success = test_binance_rest_api()
    
    # 测试WebSocket
    ws_success = test_binance_websocket()
    
    # 总结
    print("\n" + "=" * 50)
    print("📊 测试结果总结:")
    print(f"   REST API: {'✅ 成功' if rest_success else '❌ 失败'}")
    print(f"   WebSocket: {'✅ 成功' if ws_success else '❌ 失败'}")
    
    if rest_success and ws_success:
        print("\n🎉 网络连接正常，可以获取币安公开数据！")
        return True
    elif rest_success:
        print("\n⚠️ REST API正常，WebSocket可能有问题")
        return True
    else:
        print("\n❌ 网络连接有问题，无法获取币安数据")
        return False

if __name__ == "__main__":
    success = main()
    exit(0 if success else 1)